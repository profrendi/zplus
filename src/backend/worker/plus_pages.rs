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
            PlusRequest::StatusSeen(id) => {
                let _ = self.archive.plus_mark_status_seen(&id);
            }
            PlusRequest::KeepOwnStatus { id, raw } => {
                let me = self.me();
                if let Err(error) = self
                    .archive
                    .plus_add_status(&id, &me, crate::util::now(), &raw)
                {
                    log::warn!("a posted status was not kept: {error}");
                }
                self.emit_statuses();
            }
            PlusRequest::LoadStatusPrivacy => self.emit_status_privacy(),
            PlusRequest::SetStatusPrivacy { audience, people } => {
                self.set_status_privacy(audience, people)
            }
            PlusRequest::ReplyStatus { id, text } => self.plus_reply_status(id, text),
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
        let shown = status_shown(message);
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
        let seen = self.archive.plus_seen_statuses().unwrap_or_default();
        let muted = self.archive.plus_muted_status_senders().unwrap_or_default();
        let statuses = self
            .archive
            .plus_statuses()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, sender, timestamp, raw)| {
                let message = wa::Message::decode_from_slice(&raw).ok()?;
                let mut post = status_post(id, sender.clone(), timestamp, &message, sender == me);
                post.seen = seen.contains(&post.id);
                post.muted = muted.contains(&sender);
                Some(post)
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
    /// out. A JPEG goes as it is, not compressed again; a video with its
    /// first picture and length. What was posted is kept for My status.
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
        let commands = self.commands.clone();
        let waker = self.waker.clone();
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
            let caption = text.trim().to_owned();
            let caption = (!caption.is_empty()).then_some(caption);
            let sent = match picture {
                Some(path) => {
                    let bytes = tokio::fs::read(&path)
                        .await
                        .map_err(|error| error.to_string())?;
                    let video = path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| {
                            ["mp4", "m4v", "mov", "3gp", "mkv", "webm"]
                                .contains(&extension.to_ascii_lowercase().as_str())
                        });
                    if video {
                        let poster = {
                            let bytes = bytes.clone();
                            tokio::task::spawn_blocking(move || crate::animation::poster(&bytes))
                                .await
                                .ok()
                                .flatten()
                        };
                        let thumbnail = poster
                            .as_ref()
                            .and_then(|poster| poster.picture.clone())
                            .and_then(|picture| {
                                thumbnail_jpeg(&image::DynamicImage::ImageRgb8(picture))
                            })
                            .unwrap_or_default();
                        let seconds = poster.as_ref().map_or(0, |poster| poster.seconds);
                        let upload = upload_media(&client, bytes, MediaType::Video, None).await?;
                        client
                            .status()
                            .send_video(
                                upload,
                                thumbnail,
                                seconds,
                                caption.as_deref(),
                                &recipients,
                                options,
                            )
                            .await
                            .map_err(|error| error.to_string())?
                    } else {
                        let (jpeg, thumbnail) = tokio::task::spawn_blocking(move || {
                            let decoded = image::load_from_memory(&bytes)
                                .map_err(|error| error.to_string())?;
                            // A JPEG goes as it was taken: compressed again,
                            // it would only lose detail.
                            let jpeg = if bytes.starts_with(&[0xFF, 0xD8]) {
                                bytes
                            } else {
                                encode_jpeg(&decoded, 95)?
                            };
                            Ok::<_, String>((jpeg, thumbnail_jpeg(&decoded).unwrap_or_default()))
                        })
                        .await
                        .map_err(|error| error.to_string())??;
                        let upload = upload_media(&client, jpeg, MediaType::Image, None).await?;
                        client
                            .status()
                            .send_image(upload, thumbnail, caption.as_deref(), &recipients, options)
                            .await
                            .map_err(|error| error.to_string())?
                    }
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
                        .map_err(|error| error.to_string())?
                }
            };
            let _ = commands.send(Command::Plus(PlusRequest::KeepOwnStatus {
                id: sent.message_id.to_string(),
                raw: sent.message.encode_to_vec(),
            }));
            waker.wake();
            Ok("Status posted".to_owned())
        });
    }

    /// ZFPlus: the account's Status privacy, as the phone last synced it.
    fn emit_status_privacy(&self) {
        use crate::model::StatusAudience;
        use wa::sync_action_value::status_privacy_action::StatusDistributionMode as Mode;
        let Some(client) = self.client.clone() else {
            return;
        };
        let audience = client.status().audience();
        let mode = audience
            .as_ref()
            .and_then(|audience| audience.mode.as_ref())
            .and_then(|mode| mode.as_known());
        let people: Vec<String> = audience
            .as_ref()
            .map(|audience| {
                audience
                    .user_jid
                    .iter()
                    .filter_map(|jid| jid.parse::<Jid>().ok())
                    // The picker lists saved contacts by number.
                    .map(|jid| match self.lid_to_pn.get(jid.user.as_str()) {
                        Some(pn) if jid.server == "lid" => format!("{pn}@s.whatsapp.net"),
                        _ => self.canonical(&jid),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let audience = match mode {
            Some(Mode::ALLOW_LIST) => StatusAudience::Only,
            Some(Mode::DENY_LIST) => StatusAudience::Except,
            _ => StatusAudience::Contacts,
        };
        self.emit(Event::Plus(PlusAnswer::StatusPrivacy(audience, people)));
    }

    /// ZFPlus: sets who sees the account's statuses, as WhatsApp Web's
    /// Status privacy does, for the phone and every linked device.
    fn set_status_privacy(&mut self, audience: crate::model::StatusAudience, people: Vec<String>) {
        use crate::model::StatusAudience;
        use wa::sync_action_value::status_privacy_action::StatusDistributionMode as Mode;
        let jids: Vec<String> = people
            .iter()
            .filter_map(|id| Self::jid_of(id))
            .map(|jid| jid.to_string())
            .collect();
        let events = self.events.clone();
        let waker = self.waker.clone();
        let shown = people.clone();
        self.plus_task(move |client| async move {
            let mut action = wa::sync_action_value::StatusPrivacyAction::default();
            action.mode = Some(
                match audience {
                    StatusAudience::Contacts => Mode::CONTACTS,
                    StatusAudience::Except => Mode::DENY_LIST,
                    StatusAudience::Only => Mode::ALLOW_LIST,
                }
                .into(),
            );
            action.user_jid = if audience == StatusAudience::Contacts {
                Vec::new()
            } else {
                jids
            };
            let mut value = wa::SyncActionValue::default();
            value.status_privacy = Some(action).into();
            value.timestamp = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
                    .unwrap_or(0),
            );
            client
                .send_app_state_action(&whatsapp_rust::schemas::STATUS_PRIVACY, &[], &value)
                .await
                .map_err(|error| error.to_string())?;
            let _ = events.send(Event::Plus(PlusAnswer::StatusPrivacy(audience, shown)));
            waker.wake();
            Ok("Status privacy saved".to_owned())
        });
    }

    /// ZFPlus: a reply to a status, sent to its sender's chat quoting the
    /// update the way WhatsApp's own reply bar does, and filed there.
    fn plus_reply_status(&mut self, id: String, text: String) {
        use whatsapp_rust::prelude::MessageField;
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let entry = self.archive.plus_status_entry(&id).ok().flatten();
        let Some((sender, raw)) = entry else {
            self.emit(Event::Error("That status is no longer here.".to_owned()));
            return;
        };
        let Ok(quoted) = wa::Message::decode_from_slice(&raw) else {
            return;
        };
        let (Some(client), Some(jid)) = (self.connected_client(), Self::jid_of(&sender)) else {
            self.emit(Event::Error(
                "Connect to WhatsApp to reply to a status.".to_owned(),
            ));
            return;
        };
        let mut context = wa::ContextInfo::default();
        context.stanza_id = Some(id);
        context.participant = Some(jid.to_string());
        context.remote_jid = Some("status@broadcast".to_owned());
        context.quoted_message = MessageField::some(quoted);
        let mut message = outgoing_text(text.clone(), Some(context), &[]);
        let chat = sender;
        let expiration = self.apply_ephemeral(&chat, &mut message);
        let shown = self.quoted_of(&message);
        let message_id = client.generate_message_id().into_string();
        let row = Message {
            id: message_id.clone(),
            chat: chat.clone(),
            sender: self.me(),
            sender_name: None,
            from_me: true,
            timestamp: crate::util::now(),
            content: Content::text(text),
            status: Delivery::Pending,
            delivered_at: None,
            read_at: None,
            quoted: shown,
            reactions: Vec::new(),
            history_order: None,
            edited: false,
            mentions: Vec::new(),
            forwarded: false,
            thumbnail: None,
        };
        self.store_message(row, Some(message.encode_to_vec()), None);
        tokio::spawn(send_outgoing(
            client,
            self.commands.clone(),
            chat,
            jid,
            message_id,
            message,
            expiration,
            None,
        ));
        self.emit(Event::Plus(PlusAnswer::Done(Ok("Reply sent".to_owned()))));
    }

    /// ZFPlus: once a run, asks the phone for the statuses of the last day
    /// that came before the oldest kept here (those from before Status was
    /// kept), the way an older-messages request asks for a chat's.
    pub(super) fn plus_ask_older_statuses(&mut self) {
        if self.plus_statuses_asked {
            return;
        }
        let Some(client) = self.connected_client() else {
            return;
        };
        self.plus_statuses_asked = true;
        let day_ago = crate::util::now() - 24 * 60 * 60;
        let Some((id, sender, timestamp)) = self.archive.plus_oldest_status().ok().flatten() else {
            return;
        };
        if timestamp <= day_ago + 30 * 60 {
            return;
        }
        let from_me = sender == self.me();
        let Ok(jid) = "status@broadcast".parse::<Jid>() else {
            return;
        };
        tokio::spawn(async move {
            if client
                .fetch_message_history(&jid, &id, from_me, timestamp, 50)
                .await
                .is_err()
            {
                log::info!("older statuses were not asked for");
            }
        });
    }

    /// ZFPlus: once, reads the account's status settings again from
    /// WhatsApp (whose statuses are muted, and Status privacy with its
    /// lists), as an archive from before they were kept never had them.
    pub(super) fn plus_recover_status_settings(&mut self) {
        const DONE: &str = "plus_status_settings_recovered";
        if matches!(self.archive.meta(DONE), Ok(Some(_))) {
            return;
        }
        let Some(client) = self.connected_client() else {
            return;
        };
        let _ = self.archive.set_meta(DONE, "1");
        tokio::spawn(async move {
            if let Err(error) = client
                .resync_app_state(
                    [whatsapp_rust::WAPatchName::RegularHigh],
                    whatsapp_rust::AppStateResyncMode::Snapshot,
                )
                .await
            {
                log::info!("status settings were not read again: {error}");
            }
        });
    }

    /// ZFPlus: the statuses a history chunk carried, kept as live ones are.
    pub(super) fn plus_keep_history_statuses(&mut self) {
        let statuses = std::mem::take(
            &mut *HISTORY_STATUSES
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        let day_ago = crate::util::now() - 24 * 60 * 60;
        for entry in statuses {
            let Some(key) = entry.key.as_option() else {
                continue;
            };
            let (Some(id), Some(message)) = (key.id.clone(), entry.message.as_option()) else {
                continue;
            };
            let timestamp = entry
                .message_timestamp
                .and_then(|at| i64::try_from(at).ok())
                .unwrap_or(0);
            if timestamp < day_ago {
                continue;
            }
            let sender = if key.from_me.unwrap_or(false) {
                self.me()
            } else {
                match key
                    .participant
                    .as_deref()
                    .or(entry.participant.as_deref())
                    .and_then(|jid| jid.parse::<Jid>().ok())
                {
                    Some(jid) => self.canonical(&jid),
                    None => continue,
                }
            };
            if !status_shown(message) {
                continue;
            }
            if let Err(error) =
                self.archive
                    .plus_add_status(&id, &sender, timestamp, &message.encode_to_vec())
            {
                log::warn!("a status update from history was not kept: {error}");
            }
        }
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
                    let subgroups = client
                        .community()
                        .get_subgroups(&overview.id)
                        .await
                        .unwrap_or_default();
                    let announcements = subgroups
                        .iter()
                        .find(|group| group.is_default_sub_group)
                        .map(|group| group.id.to_string());
                    let groups = subgroups
                        .into_iter()
                        .map(|group| {
                            let id = group.id.to_string();
                            let inside = joined.contains(&id) || group.is_default_sub_group;
                            let name = if group.is_default_sub_group {
                                "Announcements".to_owned()
                            } else {
                                group.subject
                            };
                            (id, name, inside)
                        })
                        .collect();
                    // Its description and size, as Community info shows.
                    let metadata = client.groups().fetch_metadata(&overview.id).await.ok();
                    remember_community(&overview.id.to_string());
                    communities.push(CommunityInfo {
                        id: overview.id.to_string(),
                        name: overview.subject.unwrap_or_else(|| "Community".to_owned()),
                        groups,
                        announcements,
                        description: metadata
                            .as_ref()
                            .and_then(|metadata| metadata.description.clone())
                            .unwrap_or_default(),
                        members: metadata
                            .as_ref()
                            .map(|metadata| metadata.participants.len())
                            .filter(|count| *count > 0),
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
    let seconds = base
        .video_message
        .as_option()
        .and_then(|video| video.seconds)
        .unwrap_or(0);
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
        seen: mine,
        muted: false,
        seconds,
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

/// ZFPlus: the communities known to be ones, whose pictures are asked for by
/// the community route.
static COMMUNITIES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn remember_community(id: &str) {
    let mut known = COMMUNITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !known.iter().any(|kept| kept == id) {
        known.push(id.to_owned());
    }
}

/// ZFPlus: whether `id` is a community (its parent group).
pub(super) fn is_community(id: &str) -> bool {
    COMMUNITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .any(|kept| kept == id)
}

/// ZFPlus: statuses a history chunk carried, from the thread that unpacked
/// it to the worker that files it.
static HISTORY_STATUSES: std::sync::Mutex<Vec<wa::WebMessageInfo>> =
    std::sync::Mutex::new(Vec::new());

/// ZFPlus: sets aside a history chunk's statuses for the worker.
pub(super) fn stash_history_statuses(statuses: &[wa::WebMessageInfo]) {
    if statuses.is_empty() {
        return;
    }
    HISTORY_STATUSES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .extend(statuses.iter().cloned());
}

/// ZFPlus: whether a status update is one the page can show.
fn status_shown(message: &wa::Message) -> bool {
    let base = message.get_base_message();
    base.image_message.is_set()
        || base.video_message.is_set()
        || base.extended_text_message.is_set()
        || base.conversation.is_some()
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
