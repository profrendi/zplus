//! ZFPlus: what the Status, Channels, Communities and Business tools pages
//! ask WhatsApp for.
//!
//! Status updates are kept as WhatsApp sent them for a day, in a table of
//! their own, and are never answered with a read receipt: seeing one here
//! does not tell its sender. A photo or video is fetched only when asked.

use super::*;
use crate::model::{CatalogItem, CommunityInfo, PlusAnswer, PlusRequest, StatusKind, StatusPost};

impl Worker {
    pub(super) fn plus_request(&mut self, request: PlusRequest) {
        match request {
            PlusRequest::LoadStatuses => self.emit_statuses(),
            PlusRequest::DownloadStatus(id) => self.download_status(id),
            PlusRequest::PostStatus {
                text,
                background,
                picture,
            } => self.post_status(text, background, picture),
            PlusRequest::LoadCommunities => self.load_communities(),
            PlusRequest::CreateCommunity { name, description } => {
                self.plus_task(move |client| async move {
                    let mut options = whatsapp_rust::features::CreateCommunityOptions::new(name)
                        .map_err(|error| error.to_string())?;
                    if !description.trim().is_empty() {
                        options.description =
                            whatsapp_rust::features::GroupDescription::new(description.trim()).ok();
                    }
                    client
                        .community()
                        .create(options)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok("Community created".to_owned())
                });
                self.load_communities();
            }
            PlusRequest::JoinCommunityGroup { community, group } => {
                let (Some(community), Some(group)) =
                    (Self::jid_of(&community), Self::jid_of(&group))
                else {
                    return;
                };
                self.plus_task(move |client| async move {
                    client
                        .community()
                        .join_subgroup(community, group)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok("Joined the group".to_owned())
                });
            }
            PlusRequest::CreateChannel { name, description } => {
                self.plus_task(move |client| async move {
                    let description = description.trim();
                    let made = client
                        .newsletter()
                        .create(
                            name.trim(),
                            (!description.is_empty()).then_some(description),
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok(format!("Channel {} created", made.name))
                });
            }
            PlusRequest::FollowChannel(link) => {
                let code = link
                    .trim()
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                if code.is_empty() {
                    self.emit(Event::Plus(PlusAnswer::Done(Err(
                        "Paste a channel link, such as https://whatsapp.com/channel/…".to_owned(),
                    ))));
                    return;
                }
                self.plus_task(move |client| async move {
                    let found = client
                        .newsletter()
                        .get_metadata_by_invite(&code)
                        .await
                        .map_err(|error| error.to_string())?;
                    client
                        .newsletter()
                        .join(&found.jid)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok(format!("Following {}", found.name))
                });
            }
            PlusRequest::UnfollowChannel(channel) => {
                let Some(jid) = Self::jid_of(&channel) else {
                    return;
                };
                self.plus_task(move |client| async move {
                    client
                        .newsletter()
                        .leave(&jid)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok("Channel unfollowed".to_owned())
                });
            }
            PlusRequest::LoadCatalog => {
                let Some(me) = self.me_pn.clone().and_then(|pn| Self::jid_of(&pn)) else {
                    return;
                };
                let Some(client) = self.client.clone() else {
                    self.emit(Event::Plus(PlusAnswer::Catalog(Err(
                        "Not connected to WhatsApp".to_owned(),
                    ))));
                    return;
                };
                let (events, waker) = (self.events.clone(), self.waker.clone());
                tokio::spawn(async move {
                    let found = client
                        .business()
                        .get_catalog(&me, &Default::default())
                        .await
                        .map(|catalog| catalog.products.iter().map(catalog_item).collect())
                        .map_err(|error| error.to_string());
                    let _ = events.send(Event::Plus(PlusAnswer::Catalog(found)));
                    waker.wake();
                });
            }
        }
    }

    /// Runs a request that changes something, and says how it went.
    fn plus_task<F, Fut>(&self, task: F)
    where
        F: FnOnce(Arc<Client>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<String, String>> + Send,
    {
        let Some(client) = self.client.clone() else {
            self.emit(Event::Plus(PlusAnswer::Done(Err(
                "Not connected to WhatsApp".to_owned(),
            ))));
            return;
        };
        let (events, waker) = (self.events.clone(), self.waker.clone());
        tokio::spawn(async move {
            let result = task(client).await;
            if let Err(error) = &result {
                log::info!("a ZFPlus page request failed: {error}");
            }
            let _ = events.send(Event::Plus(PlusAnswer::Done(result)));
            waker.wake();
        });
    }

    /// A status update arrived: kept for a day, or, when its sender
    /// deleted it, forgotten.
    pub(super) fn plus_keep_status(&mut self, message: &wa::Message, info: &MessageInfo) {
        let base = message.get_base_message();
        if let Some(protocol) = base.protocol_message.as_option() {
            if protocol.r#type == Some(wa::message::protocol_message::Type::REVOKE)
                && let Some(target) = protocol.key.as_option().and_then(|key| key.id.clone())
            {
                let _ = self.archive.plus_drop_status(&target);
            }
            return;
        }
        let shown = base.image_message.is_set()
            || base.video_message.is_set()
            || base.extended_text_message.is_set()
            || base.conversation.is_some();
        if !shown {
            return;
        }
        let sender = if info.source.is_from_me {
            self.me()
        } else {
            self.canonical(&info.source.sender)
        };
        if let Err(error) = self.archive.plus_add_status(
            &info.id,
            &sender,
            info.timestamp.timestamp(),
            &message.encode_to_vec(),
        ) {
            log::warn!("a status update was not kept: {error}");
        }
    }

    fn emit_statuses(&self) {
        let me = self.me();
        let statuses = self
            .archive
            .plus_statuses()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, sender, timestamp, raw)| {
                let message = wa::Message::decode_from_slice(&raw).ok()?;
                Some(status_post(
                    id,
                    sender.clone(),
                    timestamp,
                    &message,
                    sender == me,
                ))
            })
            .collect();
        self.emit(Event::Plus(PlusAnswer::Statuses(statuses)));
    }

    fn download_status(&mut self, id: String) {
        let raw = self.archive.plus_status_raw(&id).ok().flatten();
        let message = raw.and_then(|raw| wa::Message::decode_from_slice(&raw).ok());
        let Some((client, message)) = self.client.clone().zip(message) else {
            self.emit(Event::Plus(PlusAnswer::StatusMedia {
                id,
                result: Err("This status cannot be fetched now".to_owned()),
            }));
            return;
        };
        let dir = self.dirs.media_cache_dir();
        let (events, waker) = (self.events.clone(), self.waker.clone());
        tokio::spawn(async move {
            let base = message.get_base_message();
            let result = async {
                let (bytes, extension) = if let Some(image) = base.image_message.as_option() {
                    let bytes = client
                        .download(image, Default::default())
                        .await
                        .map_err(|error| error.to_string())?;
                    (bytes, "jpg")
                } else if let Some(video) = base.video_message.as_option() {
                    let bytes = client
                        .download(video, Default::default())
                        .await
                        .map_err(|error| error.to_string())?;
                    (bytes, "mp4")
                } else {
                    return Err("This status has no photo or video".to_owned());
                };
                let _ = tokio::fs::create_dir_all(&dir).await;
                let safe: String = id
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .take(40)
                    .collect();
                let path = dir.join(format!("status-{safe}.{extension}"));
                tokio::fs::write(&path, bytes)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(path)
            }
            .await;
            let _ = events.send(Event::Plus(PlusAnswer::StatusMedia { id, result }));
            waker.wake();
        });
    }

    /// Posts a status update to the people the account's status privacy
    /// names: its contacts, those chosen, or its contacts but those left
    /// out.
    fn post_status(
        &mut self,
        text: String,
        background: [u8; 4],
        picture: Option<std::path::PathBuf>,
    ) {
        let contacts: Vec<Jid> = self
            .archive
            .contacts()
            .unwrap_or_default()
            .into_iter()
            .filter(|contact| contact.id.ends_with("@s.whatsapp.net"))
            .filter_map(|contact| Self::jid_of(&contact.id))
            .collect();
        self.plus_task(move |client| async move {
            use wa::sync_action_value::status_privacy_action::StatusDistributionMode as Mode;
            use whatsapp_rust::features::{StatusPrivacySetting, StatusSendOptions};
            let audience = client.status().audience();
            let named: Vec<Jid> = audience
                .as_ref()
                .map(|audience| {
                    audience
                        .user_jid
                        .iter()
                        .filter_map(|jid| jid.parse().ok())
                        .collect()
                })
                .unwrap_or_default();
            let mode = audience
                .as_ref()
                .and_then(|audience| audience.mode.as_ref())
                .and_then(|mode| mode.as_known());
            let (privacy, recipients) = match mode {
                Some(Mode::ALLOW_LIST) => (StatusPrivacySetting::AllowList, named),
                Some(Mode::DENY_LIST) => (
                    StatusPrivacySetting::DenyList,
                    contacts
                        .into_iter()
                        .filter(|contact| !named.contains(contact))
                        .collect(),
                ),
                _ => (StatusPrivacySetting::Contacts, contacts),
            };
            if recipients.is_empty() {
                return Err(
                    "No one to share the status with: your contacts are not here yet".to_owned(),
                );
            }
            let mut options = StatusSendOptions::default();
            options.privacy = privacy;
            match picture {
                Some(path) => {
                    let bytes = tokio::fs::read(&path)
                        .await
                        .map_err(|error| error.to_string())?;
                    let (jpeg, thumbnail) = tokio::task::spawn_blocking(move || {
                        let decoded =
                            image::load_from_memory(&bytes).map_err(|error| error.to_string())?;
                        let jpeg = encode_jpeg(&decoded, 88)?;
                        Ok::<_, String>((jpeg, thumbnail_jpeg(&decoded).unwrap_or_default()))
                    })
                    .await
                    .map_err(|error| error.to_string())??;
                    let upload = upload_media(&client, jpeg, MediaType::Image, None).await?;
                    let caption = text.trim();
                    client
                        .status()
                        .send_image(
                            upload,
                            thumbnail,
                            (!caption.is_empty()).then_some(caption),
                            &recipients,
                            options,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                }
                None => {
                    let argb = u32::from(background[3]) << 24
                        | u32::from(background[0]) << 16
                        | u32::from(background[1]) << 8
                        | u32::from(background[2]);
                    client
                        .status()
                        .send_text(
                            text.trim(),
                            argb,
                            wa::message::extended_text_message::FontType::SYSTEM,
                            &recipients,
                            options,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                }
            }
            Ok("Status posted".to_owned())
        });
    }

    fn load_communities(&self) {
        let Some(client) = self.client.clone() else {
            self.emit(Event::Plus(PlusAnswer::Communities(Err(
                "Not connected to WhatsApp".to_owned(),
            ))));
            return;
        };
        let joined: std::collections::HashSet<String> = self
            .archive
            .chats()
            .unwrap_or_default()
            .into_iter()
            .map(|chat| chat.id)
            .collect();
        let (events, waker) = (self.events.clone(), self.waker.clone());
        tokio::spawn(async move {
            let result = async {
                let overviews = client
                    .community()
                    .list_participating()
                    .await
                    .map_err(|error| error.to_string())?;
                let mut communities = Vec::new();
                for overview in overviews {
                    let groups = client
                        .community()
                        .get_subgroups(&overview.id)
                        .await
                        .unwrap_or_default()
                        .into_iter()
                        .map(|group| {
                            let id = group.id.to_string();
                            let inside = joined.contains(&id);
                            (id, group.subject, inside)
                        })
                        .collect();
                    communities.push(CommunityInfo {
                        id: overview.id.to_string(),
                        name: overview.subject.unwrap_or_else(|| "Community".to_owned()),
                        groups,
                    });
                }
                Ok(communities)
            }
            .await;
            let _ = events.send(Event::Plus(PlusAnswer::Communities(result)));
            waker.wake();
        });
    }
}

/// A status update as the page shows it.
fn status_post(
    id: String,
    sender: String,
    timestamp: i64,
    message: &wa::Message,
    mine: bool,
) -> StatusPost {
    let base = message.get_base_message();
    let thumbnail = |bytes: &Option<Vec<u8>>| {
        bytes
            .as_ref()
            .filter(|bytes| !bytes.is_empty())
            .map(|bytes| Arc::new(bytes.clone()))
    };
    let (kind, text, background, preview) = if let Some(image) = base.image_message.as_option() {
        (
            StatusKind::Image,
            image.caption.clone().unwrap_or_default(),
            None,
            thumbnail(&image.jpeg_thumbnail),
        )
    } else if let Some(video) = base.video_message.as_option() {
        (
            StatusKind::Video,
            video.caption.clone().unwrap_or_default(),
            None,
            thumbnail(&video.jpeg_thumbnail),
        )
    } else if let Some(text) = base.extended_text_message.as_option() {
        let background = text
            .background_argb
            .map(|argb| [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, 255]);
        (
            StatusKind::Text,
            text.text.clone().unwrap_or_default(),
            background,
            None,
        )
    } else if let Some(text) = &base.conversation {
        (StatusKind::Text, text.clone(), None, None)
    } else {
        (StatusKind::Other, String::new(), None, None)
    };
    StatusPost {
        id,
        sender,
        timestamp,
        kind,
        text,
        background,
        thumbnail: preview,
        mine,
    }
}

fn catalog_item(product: &whatsapp_rust::features::Product) -> CatalogItem {
    CatalogItem {
        name: product.name.clone().unwrap_or_default(),
        description: product.description.clone().unwrap_or_default(),
        price: product.price.as_ref().map(|price| {
            format!(
                "{} {:.2}",
                price.currency.clone().unwrap_or_default(),
                price.amount_1000 as f64 / 1000.0
            )
        }),
        link: product.url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A text status keeps its words and background; a photo its caption
    /// and preview.
    #[test]
    fn status_updates_are_read_for_the_page() {
        let mut text = wa::Message::default();
        text.extended_text_message = MessageField::some({
            let mut message = wa::message::ExtendedTextMessage::default();
            message.text = Some("Hello".into());
            message.background_argb = Some(0xFF00_8069);
            message
        });
        let post = status_post("a".into(), "s".into(), 10, &text, false);
        assert_eq!(post.kind, StatusKind::Text);
        assert_eq!(post.text, "Hello");
        assert_eq!(post.background, Some([0, 0x80, 0x69, 255]));
        let mut photo = wa::Message::default();
        photo.image_message = MessageField::some({
            let mut image = wa::message::ImageMessage::default();
            image.caption = Some("Look".into());
            image.jpeg_thumbnail = Some(vec![1, 2, 3]);
            image
        });
        let post = status_post("b".into(), "s".into(), 10, &photo, true);
        assert_eq!(post.kind, StatusKind::Image);
        assert_eq!(post.text, "Look");
        assert!(post.thumbnail.is_some() && post.mine);
    }
}
