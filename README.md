<p align="center">
  <img src="docs/assets/images/logo.svg" alt="Logo ZapFast" width="88" height="88">
</p>

<h1 align="center">ZapFast Plus</h1>

<p align="center"><strong>ZapFast + fitur privasi &amp; produktivitas ala WA Web Plus.</strong><br>
Aplikasi WhatsApp desktop yang ringan (Rust, tanpa browser) untuk Windows, dengan menu <em>Enhancements</em>.</p>

<p align="center">
  <a href="https://github.com/profrendi/zplus/releases/latest"><strong>Unduh rilis terbaru</strong></a> ·
  <a href="#cara-pasang">Cara pasang</a> ·
  <a href="#apa-yang-kami-tambahkan">Apa yang ditambahkan</a> ·
  <a href="#sumber-asli-dan-kredit">Sumber asli</a>
</p>

> **ZapFast Plus adalah modifikasi pribadi (fork), bukan rilis resmi ZapFast.**
> Seluruh dasar aplikasinya adalah karya [ZapFast](https://github.com/crmne/zapfast)
> oleh Carmine Paolino. Repo ini hanya menambahkan fitur di atasnya dan ikut
> setiap rilis resmi ZapFast secara otomatis.

<p align="center">
  <img src="docs/plus/private-mode.png" alt="ZapFast Plus dengan Private Mode aktif: nama, foto, dan pesan di-blur" width="820">
</p>

## Kenapa ada repo ini

ZapFast sudah sangat bagus: WhatsApp native, cepat, dan hemat RAM. Yang belum ada
adalah fitur-fitur privasi dan produktivitas yang biasa didapat dari ekstensi
browser [WA Web Plus](https://www.wawplus.com/en#benefits) untuk WhatsApp Web.
Karena ZapFast tidak memakai browser, ekstensi itu tidak bisa dipasang, jadi
fitur-fiturnya dibuat ulang langsung di kode ZapFast.

## Apa yang kami tambahkan

Semua opsi ada di **Settings → Enhancements** (paling atas), dibagi tiga grup
seperti di WA Web Plus. Setiap grup punya saklar utama dan daftar centang.

<p align="center">
  <img src="docs/plus/enhancements.png" alt="Halaman Settings → Enhancements" width="820">
</p>

### Private Mode
| Fitur | Keterangan |
|---|---|
| Blur nama kontak | Di daftar chat, hasil pencarian, header chat. Arahkan mouse untuk melihat. |
| Blur foto kontak | Foto profil di daftar chat, header, dan di dalam grup. |
| Blur pesan terakhir | Pratinjau pesan di daftar chat dan hasil pencarian. |
| Blur isi percakapan | Setiap gelembung pesan di chat yang terbuka. |
| Blur kotak ketik | Teks yang sedang diketik. |
| Sembunyikan "mengetik…" | Lawan bicara tidak melihat Anda sedang mengetik. |
| Putar voice note diam-diam | Tanda "sudah diputar" (mikrofon biru) tidak dikirim. |
| Matikan centang biru | Tanda "sudah dibaca" tidak dikirim. |
| Invisible Mode | Tidak pernah tampil "online". |
| Pulihkan pesan yang dihapus | Pesan yang dihapus pengirim untuk semua orang tetap tersimpan, diberi tanda 🚫. |
| Tombol mata 👁 di sisi kiri | Menyalakan/mematikan Private Mode sekali klik; bisa disembunyikan. |

Blur memakai efek kabur sungguhan di GPU (shader OpenGL), seperti `filter: blur()`
di WA Web Plus, bukan sekadar menutupi dengan kotak.

### Customizations
| Fitur | Keterangan |
|---|---|
| Notifikasi kontak online | Notifikasi saat kontak dari chat terbaru mulai online. |
| Notifikasi mengetik/merekam | Notifikasi saat seseorang mulai mengetik atau merekam di chat yang tidak sedang dibuka. |
| Daftar chat tidak melompat | Chat yang baru Anda kirimi pesan tetap di posisinya sampai ada balasan. |

### Extra buttons
| Fitur | Keterangan |
|---|---|
| Tombol Like 👍 | Reaksi jempol sekali klik di samping pesan. |
| Tombol Translate 🌐 | Menerjemahkan pesan ke bahasa antarmuka lewat Google Translate (hanya pesan yang diklik yang dikirim). |
| Start new chat | Pilih kode negara, ketik nomor, langsung chat tanpa menyimpan kontak. |
| Pin chat | Sematkan/lepas chat dari header. |
| Tandai dibaca/belum dibaca | Tombol di header chat. |

Belum tersedia: transkripsi voice note (butuh mesin speech-to-text) dan unduh
status (ZapFast tidak menampilkan Status).

## Cara pasang

Unduh dari [halaman Releases](https://github.com/profrendi/zplus/releases/latest):

- **Installer**: `zapfast-v…-x86_64-pc-windows-msvc.msi`. Dipasang per pengguna ke
  `%LOCALAPPDATA%\Programs\ZapFast Plus`, **tanpa hak admin**, dengan shortcut di
  Start menu dan desktop.
- **Portable**: `zapfast-v…-x86_64-pc-windows-msvc.zip`. Ekstrak di mana saja; data
  chat dan login disimpan di folder `zapfast-data` di sebelahnya.

Tidak perlu ZapFast asli terpasang. Kalau ZapFast asli juga ada di komputer,
keduanya memakai login yang sama, tetapi hanya satu yang bisa berjalan pada satu
waktu (tutup yang satu lewat ikon tray → Quit sebelum membuka yang lain).

Windows SmartScreen mungkin memberi peringatan karena file tidak ditandatangani
sertifikat: pilih *More info → Run anyway*.

## Cara update bekerja

1. **Setiap hari**, workflow [`plus-sync`](.github/workflows/plus-sync.yml) mengecek
   rilis terbaru [ZapFast resmi](https://github.com/crmne/zapfast/releases).
2. Kalau ada versi baru, rilis itu digabungkan (merge) dengan fitur Plus, lalu
   workflow [`plus-release`](.github/workflows/plus-release.yml) membangun installer
   dan versi portable, menandatangani `checksums.txt` (Ed25519), dan menerbitkan
   rilis di repo ini.
3. ZapFast Plus di komputer, baik yang di-install maupun portable, mendeteksi
   rilis baru dan memperbarui dirinya sendiri; fitur Plus tetap ada.
4. Kalau penggabungan bentrok dengan perubahan kode resmi, otomatis dibuat
   *issue* untuk diselesaikan secara manual.

Penomoran versi: `mayor.minor.(patch × 100 + revisi Plus)`. Contoh: ZapFast 0.21.1
dengan revisi Plus pertama menjadi **0.21.101**; ZapFast 0.21.2 menjadi 0.21.201.

## Untuk pengembang

- Fitur Plus: [`src/plus.rs`](src/plus.rs), [`src/ui/plus_new_chat.rs`](src/ui/plus_new_chat.rs),
  dan bagian kode bertanda komentar `ZapFast Plus:` di `src/`.
- Pembuatan paket: folder [`plus/`](plus) (build silang Windows dari Linux dengan
  MinGW, installer MSI dengan `wixl`, zip portable).
- Catatan teknis singkat: [`PLUS.md`](PLUS.md).
- Panduan asli ZapFast untuk kontributor tetap berlaku: [`AGENTS.md`](AGENTS.md),
  [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Sumber asli dan kredit

- **ZapFast**: aplikasi dasarnya. Karya Carmine Paolino, lisensi MIT.
  - Kode sumber: https://github.com/crmne/zapfast
  - Situs dan panduan: https://zapfast.rocks
  - README asli: https://github.com/crmne/zapfast#readme
- **WA Web Plus** by Elbruz Technologies: inspirasi daftar fitur. Tidak ada kode
  dari ekstensi tersebut yang dipakai; fiturnya dibuat ulang dari awal.
  - https://www.wawplus.com
  - https://chromewebstore.google.com/detail/wa-web-plus-by-elbruz-tec/ekcgkejcjdcmonfpmnljobemcbpnkamh
- **whatsapp-rust**: pustaka protokol WhatsApp yang dipakai ZapFast,
  https://github.com/oxidezap/whatsapp-rust

## Lisensi dan penafian

Mengikuti lisensi ZapFast: [MIT](LICENSE). Hak cipta asli tetap milik
Carmine Paolino; daftar komponen pihak ketiga ada di
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

ZapFast Plus tidak berafiliasi dengan WhatsApp, Meta, ZapFast, maupun WA Web Plus.
Fitur seperti menyembunyikan status online atau centang biru hanya mengubah apa
yang dikirim aplikasi ini; gunakan dengan bijak.
