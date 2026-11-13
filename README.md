<p align="center">
  <img src="docs/plus/logo.svg" alt="ZFPlus logo" width="96" height="96">
</p>

<h1 align="center">ZFPlus</h1>

<p align="center"><strong>ZapFast + the privacy and productivity features of WA Web Plus.</strong><br>
<strong>ZapFast + fitur privasi dan produktivitas ala WA Web Plus.</strong></p>

<p align="center">
  <a href="https://github.com/profrendi/zplus/releases/latest"><strong>Download / Unduh</strong></a>
</p>

<p align="center"><em>Choose your language below · Pilih bahasa di bawah ini</em></p>

<details open>
<summary><b>English (click to open or close)</b></summary>

> **ZFPlus (ZapFast Plus) is a personal fork, not an official ZapFast release.**
> Everything it is built on is [ZapFast](https://github.com/crmne/zapfast) by
> Carmine Paolino. This repository only adds features on top, and follows every
> official ZapFast release automatically.

<p align="center">
  <img src="docs/plus/private-mode.png" alt="ZFPlus with Private Mode on: names, pictures and messages are blurred" width="820">
</p>

### Why this exists

ZapFast is already great: native WhatsApp, fast, and light on memory. What it
lacks are the privacy and productivity tools people get from the
[WA Web Plus](https://www.wawplus.com/en#benefits) browser extension for
WhatsApp Web. ZapFast has no browser, so the extension can't be installed —
instead, its features were rebuilt directly in ZapFast's code.

### What we added

Every option lives in **Settings → Enhancements** (at the top), in three groups
like WA Web Plus. Each group has a main switch and a list of checkboxes.

<p align="center">
  <img src="docs/plus/enhancements.png" alt="Settings → Enhancements" width="820">
</p>

#### Private Mode
| Feature | What it does |
|---|---|
| Blur contact names | In the chat list, search results and chat header. Hover to reveal. |
| Blur contact photos | Profile pictures in the chat list, header and group chats. |
| Blur recent messages | The last-message preview in the chat list and search results. |
| Blur conversation messages | Every message bubble in the open chat. |
| Blur message composer | What you're typing. |
| Hide "typing…" | Others don't see you typing. |
| Play voice messages privately | The "played" receipt (blue microphone) isn't sent. |
| Disable read receipts | No blue ticks are sent. |
| Invisible Mode | You never show as "online". |
| Restore deleted messages | Messages deleted for everyone stay visible, marked with 🚫. |
| Eye button 👁 in the left rail | Turns Private Mode on and off in one click; can be hidden. |
| Lock when minimized | With an app lock password set, ZFPlus locks as soon as it's minimized or closed to the tray. |
| Lock when inactive | Locks after another window has been in front for 1–10 minutes (pick with − and +). |
| Lock button | A lock button above Settings in the left rail locks ZFPlus at once. |

The blur is a real GPU blur (an OpenGL shader), like `filter: blur()` in
WA Web Plus — not just a grey box on top.

#### Customizations
| Feature | What it does |
|---|---|
| Notify when contacts come online | A notification when someone from your recent chats comes online. |
| Notify about typing and recording | A notification when someone starts typing or recording in a chat you don't have open. |
| Chat list doesn't jump | A chat you just sent to keeps its place until someone replies. |
| Install updates automatically | Downloads a new release, installs it and restarts by itself (it waits while you're in a call, recording or typing). |
| About and updates | The ⓘ button above Settings shows what ZFPlus is, links to this repository and the original, and checks, downloads and installs updates on the spot. Settings → System has the same "Check for updates now". |
| Tidier left rail | Archived sits under Chats, Starred under Calls, and Chats shows how many chats are unread, the same number as the taskbar. |
| Lighter on old computers | An open chat draws only the messages on screen, the window repaints only when something changes, search skips one-letter scans, sync runs below the window's priority, and photos are decoded at bubble size on two background threads. |
| Diagnostics | Settings → Diagnostics → Record performance writes CPU and frame times once a second (`prof.csv`, `prof-summary.txt`; numbers only, no messages, names or phone numbers; the CSV is capped at 50 MB, two files kept). F12 shows the numbers live, Copy performance report puts the summary on the clipboard. Off by default. |
| Smooth Private Mode blur | The blur is a two-pass Gaussian on the graphics card, as smooth as WA Web Plus's, with a frosted veil where the card refuses it. |
| Updates from the tray | The tray menu has Check for updates and, while there is one, Update and restart (with the download's progress). A new update is also announced in a notification that opens the update controls. |
| Feature check | Settings → Diagnostics → Feature check lists the link, the account kind, the blur, the tray, notifications, starting with the computer and updates on this computer, to copy and paste. Every release first checks that each ZFPlus feature is still on screen. |
| Why a logout happened | When WhatsApp ends the link, the log records its reason (removed from Linked devices, a server-side logout, a locked account) and the window shows WhatsApp's own explanation when it sent one. |
| WhatsApp Business | A "Business" or "Personal" mark beside Chats says which kind of account is linked. A Business account's quick replies sync from the phone and are one click away in the attach menu. |
| Attach menu like WhatsApp | Document, Photos & videos, Camera, Audio, Contact, Send location and Create poll, in WhatsApp's order, with the account kind on top. Photos & videos and Audio open the file picker on those kinds of files only. Camera takes a photo with the computer's camera; Contact sends a contact card. |
| Draw on a picture | The picture editor draws before sending: pen, straight line, arrow, rectangle, rounded rectangle, ellipse and pixelate, in six colours and four widths, shapes filled or not; text in five type styles, plain or in a filled box; and emoji, stickers and GIF pictures. Everything placed stays its own object until the picture is sent: the Move tool picks it to drag, resize (corner handle), recolour or delete (Del), and double-clicked text opens to be changed. Undo and Redo (Ctrl+Z, Ctrl+Y) take back any of it. Shift keeps lines at even angles and shapes square, and a pen stroke drawn almost straight becomes a straight line. A GIF placed on a photo stays still, since the photo is sent as a photo. |
| Status, Channels, Communities and Business tools | From the rail: status updates of the last day seen without telling their senders, photos and videos downloaded and saved, text and photo statuses posted; channels followed by link, made and unfollowed; communities with their groups to open or join, and new ones; for a business, its catalogue, profile and lists. |
| Word art and text like Office | The photo editor's text has typefaces, sizes, bold, italic, underline and strikethrough, Office's colour palette with any colour, word art styles, and Text Fill, Text Outline and Text Effects (shadow, glow, reflection, 3D). Grids take no ground or any colour, with or without lines. |
| Lists like WhatsApp | Lists (Business labels) are WhatsApp's own: made here, on the phone or on WhatsApp Web, they show everywhere, with each chat's lists under it in the chat list. |
| Word art, pictures and grids in the photo editor | Text in six word art styles, changed again with Edit text; a picture from a file to place freely; grids (2 to 9 boxes) for collages and side by side comparisons. |
| Forward with changes | Add your own message under what you forward, or Edit, then send: the words come quoted with yours under them, and a photo opens in the editor. |
| Usage in the tray | The tray icon's tooltip shows memory (as Task Manager counts it), CPU, processes and threads, live. |
| Remove from Settings | Settings → Diagnostics opens the app's folder and uninstalls ZFPlus, with or without its data. |
| Profile like WhatsApp Web | Your picture's menu in the rail, and Settings → Account, open Profile: the photo (view large, save, change), name and About to edit, the number, WhatsApp or WhatsApp Business, this computer, and for a business its description, address, email, websites, category and hours, editable where WhatsApp allows. Message yourself is there too. |
| Send contacts like WhatsApp | Tick several contacts (You first), each marked WA or NON WA as WhatsApp says, and send them in one message. |
| Location from anywhere | Send location reads links from Google Maps (short ones too), Apple Maps, Waze, OpenStreetMap, Bing and HERE, and My current location sends where this computer is, through Windows' location setting. |
| Turn and mirror in the photo editor | Objects turn freely by their round handle (Shift: steps of 15 degrees), a quarter at a time, and mirror; the whole picture mirrors left to right or top to bottom. |
| Contact photo, large | A contact's photo in its info opens large, with Save as; Save photo saves it at full size. |
| Remove an account | The account switcher (our picture at the foot of the left rail) has a bin beside each account, which asks before removing it from this computer. |
| All your history | Settings → Customizations → Full chat history when linking asks the phone for a year of every chat at the next link, as WhatsApp for Windows does (on by default; it takes effect after linking again). A chat's ⋯ menu has Load all messages from phone, which keeps asking the phone for older pages, at the pace it answers, until it has none left. How far back the phone answers is up to WhatsApp and the phone. |
| Text after a minimize | Coming back from minimized or from the tray, the window sends its text to the graphics card again, so it never comes back dark or without letters. |

#### Extra buttons
| Feature | What it does |
|---|---|
| Like 👍 | React with a thumbs-up in one click, next to any message. |
| Translate 🌐 | Translates a message into the app's language with Google Translate (only the message you click is sent). |
| Start new chat | Pick a country code, type a number, and chat without saving the contact. |
| Pin chat | Pin or unpin the chat from its header. |
| Mark read/unread | A button in the chat header. |

Not available yet: voice message transcription (needs a speech-to-text engine)
and status downloads (ZapFast doesn't show Status).

### Install

Get the files from the [Releases page](https://github.com/profrendi/zplus/releases/latest):

- **Installer**: `zapfast-v…-x86_64-pc-windows-msvc.msi`. Installs for your user
  only, into `%LOCALAPPDATA%\Programs\ZFPlus`, **no admin rights needed**,
  with Start menu and desktop shortcuts.
- **Portable**: `zapfast-v…-x86_64-pc-windows-msvc.zip`. Unzip it anywhere; your
  chats and login are kept in a `zapfast-data` folder next to it.

You don't need the original ZapFast installed. If it is, both share the same
login, but only one can run at a time (quit one from its tray icon → Quit
before opening the other).

Windows SmartScreen may warn you because the files aren't signed with a
certificate: choose *More info → Run anyway*.

### How updates work

1. **Every 6 hours**, the [`plus-sync`](.github/workflows/plus-sync.yml) workflow
   checks for a new [official ZapFast release](https://github.com/crmne/zapfast/releases).
2. If there is one, it's merged with the Plus features, and the
   [`plus-release`](.github/workflows/plus-release.yml) workflow builds the
   installer and the portable zip, signs `checksums.txt` (Ed25519) and
   publishes a release here.
3. ZFPlus on your computer — installed or portable — sees the new
   release and updates itself, keeping the Plus features.
4. If the merge clashes with the official code, an issue is opened so it can be
   resolved by hand.

Version numbers: `major.minor.(patch × 100 + Plus revision)`. For example,
ZapFast 0.21.1 with the first Plus revision is **0.21.101**; ZapFast 0.21.2
becomes 0.21.201.

### ZFPlus Nightly

A second, separate app for trying what ZapFast is working on before it is
released. It has its own name, chats, login and install folder
(`%LOCALAPPDATA%\Programs\ZFPlus Nightly`), so it installs and runs beside
ZFPlus without touching it, and it shows up as "ZFPlus Nightly" under the
phone's Linked devices. It may have bugs.

- **Once a day**, the [`plus-nightly`](.github/workflows/plus-nightly.yml)
  workflow merges ZFPlus main with ZapFast's main branch, runs the tests,
  builds `zfplus-nightly-v…-x86_64-pc-windows-msvc.msi` and `.zip` and
  publishes them as a pre-release. Nothing new on either side means no
  nightly; a conflict or a failing test opens an issue and publishes nothing.
- ZFPlus never sees these pre-releases, and ZFPlus Nightly only ever installs
  nightlies. Their versions read `0.<yyyymmdd>.<hhmm>-nightly`, shown in the
  app as the date, such as "Nightly 10 Nov 2026, 17:41".
- Both apps' About shows the newest ZFPlus and the newest ZFPlus Nightly,
  with a link to get the other one. Copy performance report names the engine
  each was built on (ZapFast, whatsapp-rust, fastframe, egui, Rust).

### For developers

- Plus features: [`src/plus.rs`](src/plus.rs), [`src/ui/plus_new_chat.rs`](src/ui/plus_new_chat.rs),
  and the code marked with `ZFPlus:` comments in `src/`.
- Packaging: the [`plus/`](plus) folder (Windows cross-build from Linux with
  MinGW, MSI installer with `wixl`, portable zip).
- Short technical notes: [`PLUS.md`](PLUS.md).
- ZapFast's own contributor guides still apply: [`AGENTS.md`](AGENTS.md),
  [`CONTRIBUTING.md`](CONTRIBUTING.md).

### Original sources and credits

- **ZapFast** — the app this is built on. By Carmine Paolino, MIT licence.
  - Source code: https://github.com/crmne/zapfast
  - Website and guides: https://zapfast.rocks
  - Original README: https://github.com/crmne/zapfast#readme
- **WA Web Plus** by Elbruz Technologies — the inspiration for the feature list.
  No code from the extension is used; the features were rebuilt from scratch.
  - https://www.wawplus.com
  - https://chromewebstore.google.com/detail/wa-web-plus-by-elbruz-tec/ekcgkejcjdcmonfpmnljobemcbpnkamh
- **whatsapp-rust** — the WhatsApp protocol library ZapFast uses:
  https://github.com/oxidezap/whatsapp-rust

### Licence and disclaimer

Same licence as ZapFast: [MIT](LICENSE). The original copyright stays with
Carmine Paolino; third-party components are listed in
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

ZFPlus is not affiliated with WhatsApp, Meta, ZapFast or WA Web Plus.
Features like hiding your online status or blue ticks only change what this
app sends — use them considerately.

</details>

<details>
<summary><b>Bahasa Indonesia (klik untuk membuka atau menutup)</b></summary>

> **ZFPlus (ZapFast Plus) adalah fork pribadi, bukan rilis resmi ZapFast.**
> Seluruh dasarnya adalah [ZapFast](https://github.com/crmne/zapfast) karya
> Carmine Paolino. Repo ini hanya menambahkan fitur di atasnya, dan otomatis
> mengikuti setiap rilis resmi ZapFast.

<p align="center">
  <img src="docs/plus/private-mode.png" alt="ZFPlus dengan Private Mode aktif: nama, foto, dan pesan di-blur" width="820">
</p>

### Kenapa repo ini dibuat

ZapFast sudah sangat bagus: WhatsApp native, cepat, dan hemat memori. Yang
belum ada hanyalah fitur privasi dan produktivitas seperti di ekstensi browser
[WA Web Plus](https://www.wawplus.com/en#benefits) untuk WhatsApp Web. Karena
ZapFast tidak memakai browser, ekstensi itu tidak bisa dipasang. Jadi,
fitur-fiturnya dibuat ulang langsung di dalam kode ZapFast.

### Fitur yang ditambahkan

Semua pengaturan ada di **Settings → Enhancements** (paling atas), dibagi
menjadi tiga grup seperti di WA Web Plus. Setiap grup punya saklar utama dan
daftar pilihan yang bisa dicentang.

<p align="center">
  <img src="docs/plus/enhancements.png" alt="Halaman Settings → Enhancements" width="820">
</p>

#### Private Mode
| Fitur | Fungsinya |
|---|---|
| Blur nama kontak | Di daftar chat, hasil pencarian, dan header chat. Arahkan mouse ke situ untuk melihat. |
| Blur foto kontak | Foto profil di daftar chat, header, dan chat grup. |
| Blur pesan terakhir | Pratinjau pesan terakhir di daftar chat dan hasil pencarian. |
| Blur isi percakapan | Semua gelembung pesan di chat yang sedang dibuka. |
| Blur kotak ketik | Teks yang sedang kamu ketik. |
| Sembunyikan "mengetik…" | Orang lain tidak melihat kamu sedang mengetik. |
| Putar voice note diam-diam | Tanda "sudah diputar" (mikrofon biru) tidak dikirim. |
| Matikan centang biru | Tanda "sudah dibaca" tidak dikirim. |
| Invisible Mode | Kamu tidak pernah terlihat "online". |
| Pulihkan pesan yang dihapus | Pesan yang dihapus untuk semua orang tetap bisa dibaca, dengan tanda 🚫. |
| Tombol mata 👁 di sisi kiri | Menyalakan atau mematikan Private Mode dengan sekali klik; tombolnya juga bisa disembunyikan. |
| Kunci saat diminimize | Kalau password app lock sudah dipasang, ZFPlus langsung terkunci saat diminimize atau ditutup ke tray. |
| Kunci saat tidak aktif | Terkunci setelah jendela lain ada di depan selama 1–10 menit (atur dengan tombol − dan +). |
| Tombol kunci | Tombol gembok di atas Settings di sisi kiri langsung mengunci ZFPlus. |

Efek blur-nya benar-benar mengaburkan tampilan lewat GPU (shader OpenGL),
sama seperti `filter: blur()` di WA Web Plus, bukan sekadar ditutup kotak abu-abu.

#### Customizations
| Fitur | Fungsinya |
|---|---|
| Notifikasi kontak online | Muncul notifikasi saat kontak dari chat terbaru mulai online. |
| Notifikasi mengetik/merekam | Muncul notifikasi saat seseorang mulai mengetik atau merekam di chat yang tidak sedang kamu buka. |
| Daftar chat tidak melompat | Chat yang baru kamu kirimi pesan tetap di posisinya sampai ada balasan. |
| Install update otomatis | Rilis baru diunduh, dipasang, lalu aplikasinya restart sendiri (menunggu dulu kalau kamu sedang telepon, merekam, atau mengetik). |
| About dan update | Tombol ⓘ di atas Settings menampilkan keterangan ZFPlus, link ke repo ini dan repo aslinya, serta bisa cek, unduh, dan pasang update saat itu juga. Settings → System juga punya "Check for updates now". |
| Sisi kiri lebih rapi | Archived ada di bawah Chats, Starred di bawah Calls, dan Chats menampilkan jumlah chat belum dibaca, sama dengan angka di taskbar. |
| Blur Private Mode halus | Blur Gaussian dua tahap di kartu grafis, sehalus WA Web Plus; kalau kartu grafis menolak, diganti lapisan buram. |
| Update dari tray | Menu tray punya Check for updates dan, saat ada pembaruan, Update and restart (dengan progres unduhan). Pembaruan baru juga diumumkan lewat notifikasi yang membuka panel update. |
| Feature check | Settings → Diagnostics → Feature check menampilkan status koneksi, jenis akun, blur, tray, notifikasi, start bersama komputer, dan update, untuk disalin. Setiap rilis dicek dulu bahwa semua fitur ZFPlus masih ada di layar. |
| Alasan logout | Saat WhatsApp memutus tautan, log mencatat alasannya (dihapus dari Perangkat tertaut, logout dari server, akun dikunci) dan jendela menampilkan penjelasan WhatsApp kalau ada. |
| WhatsApp Business | Tanda "Business" atau "Personal" di samping Chats menunjukkan jenis akun. Quick replies akun Business tersinkron dari HP dan bisa dipakai dari menu lampiran. |
| Menu lampiran seperti WhatsApp | Document, Photos & videos, Camera, Audio, Contact, Send location, dan Create poll, urut seperti WhatsApp, dengan jenis akun di atasnya. Photos & videos dan Audio membuka pemilih file khusus jenis file itu saja. Camera memotret pakai kamera komputer; Contact mengirim kartu kontak. |
| Menggambar di foto | Editor foto bisa menggambar sebelum kirim: pena, garis lurus, panah, kotak, kotak sudut bulat, elips, dan piksel (sensor), 6 warna, 4 ketebalan, bentuk bisa diisi warna; teks dengan 5 gaya huruf, polos atau dalam kotak berwarna; serta emoji, stiker, dan gambar GIF. Semua yang dimasukkan tetap jadi objek sendiri sampai foto dikirim: alat Move untuk memilih, menggeser, mengubah ukuran (pegangan di sudut), mengganti warna, atau menghapus (Del), dan teks bisa diklik dua kali untuk diubah. Undo dan Redo (Ctrl+Z, Ctrl+Y) bisa membatalkan semuanya. Shift membuat garis bersudut rapi dan bentuk jadi persegi/lingkaran; coretan pena yang hampir lurus otomatis jadi garis lurus. GIF yang ditaruh di foto jadi gambar diam, karena yang dikirim tetap foto. |
| Status, Channels, Communities, dan Business tools | Dari rel kiri: status 24 jam terakhir dilihat tanpa memberi tahu pengirimnya, foto dan video bisa diunduh dan disimpan, status teks dan foto bisa dibuat; channel diikuti lewat link, dibuat, dan berhenti diikuti; community beserta grupnya untuk dibuka atau diikuti, dan membuat community baru; untuk bisnis: katalog, profil, dan list. |
| Word art dan teks seperti Office | Teks di editor foto punya jenis huruf, ukuran, tebal, miring, garis bawah, coret, palet warna Office dengan warna bebas, gaya word art, serta Text Fill, Text Outline, dan Text Effects (bayangan, glow, pantulan, 3D). Grid bisa tanpa latar atau warna apa pun, dengan atau tanpa garis. |
| List seperti WhatsApp | List (label Business) milik WhatsApp sendiri: dibuat di sini, di HP, atau di WhatsApp Web, tampil di semuanya, dan nama list tiap chat tampil di bawahnya. |
| Word art, gambar, dan grid di editor foto | Teks dengan enam gaya word art, bisa diubah lagi lewat Edit text; gambar dari file yang bisa digeser bebas; grid (2 sampai 9 kotak) untuk kolase dan perbandingan. |
| Forward dengan perubahan | Tambahkan pesan sendiri di bawah yang di-forward, atau Edit, then send: teks asli dikutip dan komentar Anda di bawahnya, foto terbuka di editor. |
| Pemakaian di tray | Tooltip ikon tray menampilkan memori (seperti hitungan Task Manager), CPU, proses, dan thread secara langsung. |
| Hapus dari Settings | Settings → Diagnostics membuka folder aplikasi dan meng-uninstall ZFPlus, dengan atau tanpa datanya. |
| Profil seperti WhatsApp Web | Menu foto Anda di rel kiri, dan Settings → Account, membuka Profile: foto (lihat besar, simpan, ganti), nama dan About yang bisa diubah, nomor, WhatsApp atau WhatsApp Business, komputer ini, dan untuk bisnis: deskripsi, alamat, email, situs, kategori, dan jam buka, bisa diubah sejauh WhatsApp mengizinkan. Ada juga Message yourself. |
| Kirim kontak seperti WhatsApp | Centang beberapa kontak (Anda paling atas), masing-masing bertanda WA atau NON WA sesuai jawaban WhatsApp, lalu kirim sekaligus dalam satu pesan. |
| Lokasi dari mana saja | Send location membaca link Google Maps (termasuk link pendek), Apple Maps, Waze, OpenStreetMap, Bing, dan HERE, dan My current location mengirim lokasi komputer ini lewat pengaturan lokasi Windows. |
| Putar dan balik di editor foto | Objek bisa diputar bebas lewat pegangan bulatnya (Shift: per 15 derajat), per 90 derajat, dan dibalik; seluruh foto bisa dibalik kiri-kanan atau atas-bawah. |
| Foto kontak besar | Foto kontak di info kontak terbuka besar dengan Save as; Save photo menyimpan versi ukuran penuh. |
| Hapus akun | Pengalih akun (foto kita di bawah rel kiri) punya tombol tempat sampah di tiap akun, dengan konfirmasi sebelum akun dihapus dari komputer ini. |
| Semua riwayat chat | Settings → Customizations → Full chat history when linking meminta HP mengirim riwayat setahun semua chat saat menautkan, seperti WhatsApp for Windows (aktif bawaan; berlaku setelah menautkan ulang). Menu ⋯ di chat punya Load all messages from phone, yang terus meminta halaman lebih lama ke HP, mengikuti kecepatan HP menjawab, sampai habis. Seberapa jauh HP mau mengirim tetap ditentukan WhatsApp dan HP. |
| Teks setelah minimize | Saat jendela dibuka lagi dari minimize atau dari tray, teksnya dikirim ulang ke kartu grafis, jadi jendela tidak kembali gelap atau tanpa huruf. |

#### Extra buttons
| Fitur | Fungsinya |
|---|---|
| Tombol Like 👍 | Kasih reaksi jempol sekali klik di samping pesan. |
| Tombol Translate 🌐 | Menerjemahkan pesan ke bahasa aplikasi lewat Google Translate (hanya pesan yang kamu klik yang dikirim). |
| Start new chat | Pilih kode negara, ketik nomornya, lalu langsung chat tanpa perlu menyimpan kontak. |
| Pin chat | Sematkan atau lepas sematan chat langsung dari header. |
| Tandai dibaca/belum dibaca | Tombol di header chat. |

Yang belum tersedia: transkripsi voice note (butuh mesin speech-to-text) dan
unduh status (ZapFast memang tidak menampilkan Status).

### Cara pasang

Unduh file-nya dari [halaman Releases](https://github.com/profrendi/zplus/releases/latest):

- **Installer**: `zapfast-v…-x86_64-pc-windows-msvc.msi`. Terpasang khusus untuk
  akun Windows kamu di `%LOCALAPPDATA%\Programs\ZFPlus`, **tanpa perlu hak
  admin**, lengkap dengan shortcut di Start menu dan desktop.
- **Portable**: `zapfast-v…-x86_64-pc-windows-msvc.zip`. Ekstrak di folder mana
  saja; data chat dan login disimpan di folder `zapfast-data` di sebelahnya.

ZapFast asli tidak perlu terpasang. Kalau ternyata sudah ada, keduanya memakai
login yang sama, tapi hanya satu yang bisa berjalan dalam satu waktu. Tutup
salah satunya dulu (ikon tray → Quit) sebelum membuka yang lain.

Windows SmartScreen mungkin memberi peringatan karena file-nya tidak memakai
sertifikat tanda tangan digital. Pilih *More info → Run anyway*.

### Cara kerja update

1. **Setiap 6 jam**, workflow [`plus-sync`](.github/workflows/plus-sync.yml)
   mengecek apakah ada [rilis resmi ZapFast](https://github.com/crmne/zapfast/releases) yang baru.
2. Kalau ada, rilis itu digabungkan dengan fitur Plus. Setelah itu workflow
   [`plus-release`](.github/workflows/plus-release.yml) membuat installer dan
   versi portable, menandatangani `checksums.txt` (Ed25519), lalu menerbitkan
   rilisnya di repo ini.
3. ZFPlus di komputer kamu, baik versi installer maupun portable,
   mendeteksi rilis baru itu dan memperbarui dirinya sendiri. Fitur Plus tetap ada.
4. Kalau penggabungannya bentrok dengan perubahan di kode resmi, sebuah
   *issue* otomatis dibuat supaya bisa dibereskan secara manual.

Aturan nomor versi: `mayor.minor.(patch × 100 + revisi Plus)`. Contohnya,
ZapFast 0.21.1 dengan revisi Plus pertama menjadi **0.21.101**, dan ZapFast
0.21.2 nantinya menjadi 0.21.201.

### Untuk pengembang

- Kode fitur Plus: [`src/plus.rs`](src/plus.rs), [`src/ui/plus_new_chat.rs`](src/ui/plus_new_chat.rs),
  serta bagian kode yang diberi komentar `ZFPlus:` di folder `src/`.
- Pembuatan paket: folder [`plus/`](plus) (build Windows dari Linux dengan
  MinGW, installer MSI dengan `wixl`, dan zip portable).
- Catatan teknis singkat: [`PLUS.md`](PLUS.md).
- Panduan kontributor dari ZapFast asli tetap berlaku: [`AGENTS.md`](AGENTS.md),
  [`CONTRIBUTING.md`](CONTRIBUTING.md).

### Sumber asli dan kredit

- **ZapFast**: aplikasi yang menjadi dasar repo ini. Karya Carmine Paolino, berlisensi MIT.
  - Kode sumber: https://github.com/crmne/zapfast
  - Situs dan panduan: https://zapfast.rocks
  - README asli: https://github.com/crmne/zapfast#readme
- **WA Web Plus** dari Elbruz Technologies: inspirasi daftar fiturnya. Tidak ada
  kode dari ekstensi itu yang dipakai; semua fitur dibuat ulang dari nol.
  - https://www.wawplus.com
  - https://chromewebstore.google.com/detail/wa-web-plus-by-elbruz-tec/ekcgkejcjdcmonfpmnljobemcbpnkamh
- **whatsapp-rust**: pustaka protokol WhatsApp yang dipakai ZapFast,
  https://github.com/oxidezap/whatsapp-rust

### Lisensi dan penafian

Lisensinya sama dengan ZapFast: [MIT](LICENSE). Hak cipta aslinya tetap milik
Carmine Paolino; daftar komponen pihak ketiga ada di
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

ZFPlus tidak berafiliasi dengan WhatsApp, Meta, ZapFast, maupun WA Web Plus.
Fitur seperti menyembunyikan status online atau centang biru hanya mengatur apa
yang dikirim oleh aplikasi ini, jadi gunakan dengan bijak, ya.

</details>
