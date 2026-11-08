ZFPlus Prof: build pengukuran
==============================

Ini ZFPlus yang sama, ditambah pencatat pemakaian CPU. Tidak ada fitur
yang diubah. Gunanya hanya untuk mencari tahu bagian mana yang membuat
CPU tinggi, sebelum membuat ZFPlus Lite.

Yang perlu diketahui
- Datanya terpisah: login, chat, dan pengaturan disimpan di folder
  "zfplus-prof-data" di sebelah zfplus-prof.exe. ZFPlus yang terpasang
  tidak tersentuh.
- Karena datanya terpisah, build ini perlu ditautkan sebagai perangkat
  baru (scan QR atau kode nomor HP), seperti pertama kali memakai ZFPlus.
  Setelah selesai, perangkat ini bisa dihapus dari WhatsApp di HP
  (Perangkat tertaut).
- Build ini tidak memperbarui dirinya sendiri.

Cara mengukur
1. Tutup ZFPlus yang biasa dulu (ikon tray -> Quit), supaya angkanya
   tidak tercampur.
2. Jalankan zfplus-prof.exe dan tautkan akunnya. Tunggu sampai sinkron
   selesai (daftar chat tidak bergerak lagi), lalu mulai dari langkah 3.
3. Pakai seperti biasa, kira-kira 2-3 menit untuk tiap keadaan:
   a. diam di homepage tanpa menyentuh apa pun
   b. buka satu chat dan scroll
   c. cari sesuatu di kotak pencarian
   d. jendela terbuka tapi tertutup jendela lain
   e. di-minimize
4. Tekan F12 kapan saja untuk melihat angka langsung di pojok kanan bawah.
   Tekan F12 lagi untuk menutupnya.
5. Tutup aplikasinya, lalu kirim dua file dari folder zfplus-prof-data:
     prof-summary.txt   (ringkasan, ini yang paling penting)
     prof.csv           (rincian per detik)

Kedua file itu hanya berisi angka, waktu, dan nama bagian program. Isi
pesan, nama kontak, dan nomor HP tidak dicatat.
