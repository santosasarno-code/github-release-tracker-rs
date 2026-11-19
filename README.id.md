# GitHub Release Tracker (edisi Rust)

Aplikasi Windows kecil dan portabel untuk memantau rilis terbaru dari proyek GitHub yang kamu ikuti, dan memberi tahu saat ada rilis baru. Tanpa akun, tanpa login, tanpa token.

> **Catatan rewrite:** proyek ini adalah tulis ulang total, dengan Rust dan antarmuka Win32 native, dari aplikasi Python / tkinter saya sebelumnya: **[github-release-tracker](https://github.com/santosasarno-code/github-release-tracker)**. Fiturnya sama dan memakai `projects.json` yang sama, jadi kamu bisa berpindah antar keduanya tanpa kehilangan daftar. Versi Python tetap tersedia.

Berupa satu `.exe` kecil tanpa instalasi.

🇬🇧 [Read in English](README.md)

## Fitur

- Tambah proyek dengan menempel URL GitHub atau `owner/repo`
- Lihat nama proyek, repositori, rilis saat ini, tanggal terbit, dan status sekaligus
- **Check for updates** menyegarkan semua proyek; rilis yang lebih baru ditandai `Update available: v1.0 -> v1.1`
- **Mark as seen** menghapus tanda setelah kamu selesai dengan pembaruan itu
- Klik dua kali baris proyek (atau **Open release page**) untuk membuka halaman rilisnya; klik dua kali pada nama di kolom **Project** akan me-rename
- **Ganti nama** proyek: klik ikon pensil yang muncul saat kursor di atas proyek, atau pilih lalu tekan **F2**, atau klik kanan > Rename, atau klik dua kali pada nama. *Reset name* mengembalikan nama dari GitHub
- Klik header **Project**, **Repository**, **Published**, atau **Status** untuk mengurutkan (klik lagi untuk membalik)
- **Import / Export** daftar sebagai `.json` (gabung atau ganti saat import; token tidak pernah diekspor)
- Mode **terang / gelap**: klik ikon bulan / matahari di kanan bawah (awalnya mengikuti tema Windows)
- Lebar kolom ikut proporsional saat jendela diubah ukurannya; seret batas kolom untuk mengubah proporsi
- Mendukung DPI tinggi per monitor; garis tipis pemisah antar baris dan kolom
- Portabel: satu `.exe`, tanpa installer, tanpa registry. Data berupa `projects.json` di sebelah exe (cadangan di `%APPDATA%\GitHubReleaseTracker` bila foldernya hanya-baca)

Aplikasi hanya membaca informasi rilis. Tidak mengunduh atau memasang apa pun.

## Yang kamu butuhkan

| Saya ingin... | Yang dibutuhkan |
|---|---|
| **Memakai aplikasinya** | Windows 10 atau 11 dan koneksi internet. Tidak ada yang lain. |
| **Membuat `.exe` sendiri** (`build.bat`) | [Rust](https://rustup.rs) (pilihan bawaan "MSVC") dan **Build Tools for Visual Studio** dengan workload *Desktop development with C++* (sudah termasuk Windows SDK). Internet untuk build pertama (mengunduh beberapa crate). |

## Cara build

1. Pasang dua hal di atas, lalu buka jendela **baru**.
2. Klik dua kali `build.bat`.
3. Hasilnya ada di `dist\`:
   - `GitHubReleaseTracker.exe`
   - `GitHubReleaseTracker.exe.sha256`
   - `release-notes-vX.Y.Z.md` (tempel ke release GitHub)

Langkah rilis lengkap ada di [RELEASING.md](RELEASING.md), termasuk opsi GitHub Actions.

Manual: `cargo test --lib` menjalankan tes logika, `cargo build --release` menghasilkan `target\release\github-release-tracker.exe`.

## Pengaturan dan data

- `projects.json` kompatibel dengan versi Python di kedua arah (nama kustom diabaikan oleh versi Python lebih lama dari 1.1.0).
- Token GitHub opsional (Settings) disimpan sebagai teks biasa di `projects.json` dan tidak pernah diekspor. Pemakaian biasa tidak membutuhkannya: rilis dibaca dari feed publik GitHub.

## Lisensi

MIT. Lihat [LICENSE](LICENSE).
