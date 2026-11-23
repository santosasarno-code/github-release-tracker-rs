# GitHub Release Tracker (edisi Rust)

Aplikasi Windows kecil dan portabel untuk memantau rilis terbaru dari proyek yang kamu ikuti (GitHub, GitLab, Codeberg, Gitea / Forgejo, dan server self-hosted), dan memberi tahu saat ada rilis baru. Tanpa akun, tanpa login, tanpa token.

> **Catatan rewrite:** proyek ini adalah tulis ulang total, dengan Rust dan antarmuka Win32 native, dari aplikasi Python / tkinter saya sebelumnya: **[github-release-tracker](https://github.com/santosasarno-code/github-release-tracker)**. Fiturnya sama dan memakai `projects.json` yang sama, jadi kamu bisa berpindah antar keduanya tanpa kehilangan daftar. Versi Python tetap tersedia.

Berupa satu `.exe` kecil tanpa instalasi.

🇬🇧 [Read in English](README.md)

## Fitur

- Tambah proyek dengan menempel URL GitHub atau `owner/repo`
- **Host lain, caranya sama.** Tempel URL proyek di **GitLab** (`gitlab.com` atau server sendiri, termasuk grup dan subgrup), **Codeberg**, **Gitea**, **Forgejo**, atau server self-hosted dari semuanya. Hanya repo publik, tanpa token. Hanya **rilis resmi** yang dipantau: proyek yang hanya punya tag (atau belum punya apa-apa) ditolak saat ditambahkan
- Lihat nama proyek, repositori, rilis saat ini, tanggal terbit, dan status sekaligus
- **Check for updates** menyegarkan semua proyek; rilis yang lebih baru ditandai `Update available: v1.0 -> v1.1`
- **Mark as seen** menghapus tanda setelah kamu selesai dengan pembaruan itu
- Klik dua kali baris proyek (atau **Open release page**) untuk membuka halaman rilisnya; klik dua kali pada nama di kolom **Project** akan me-rename
- **Ganti nama** proyek: klik ikon pensil yang muncul saat kursor di atas proyek, atau pilih lalu tekan **F2**, atau klik kanan > Rename, atau klik dua kali pada nama. *Reset name* mengembalikan nama dari GitHub
- Klik header **Project**, **Repository**, **Source**, **Published**, atau **Status** untuk mengurutkan (klik lagi untuk membalik)
- Kolom **Source** menunjukkan asal tiap proyek (GitHub, GitLab, Codeberg, atau alamat server Anda sendiri); klik headernya untuk mengurutkan. Kotak pencarian juga mencocokkannya.
- **Filter** daftar: ketik di kotak pencarian (Ctrl+F untuk pindah ke sana, Esc untuk mengosongkan) agar hanya proyek yang namanya atau repositorinya memuat semua kata yang diketik yang tampil, dan/atau pilih *Show:* All projects / Updates available / Errors / Up to date / No releases. Ini hanya mengubah tampilan; tidak ada yang dihapus atau disimpan berbeda. Kotak menampilkan "n of m".
- **Import / Export** daftar sebagai `.json` (gabung atau ganti saat import; token tidak pernah diekspor)
- Mode **terang / gelap**: klik ikon bulan / matahari di kanan bawah (awalnya mengikuti tema Windows)
- Lebar kolom ikut proporsional saat jendela diubah ukurannya; seret batas kolom untuk mengubah proporsi
- Mendukung DPI tinggi per monitor; garis tipis pemisah antar baris dan kolom
- Portabel: satu `.exe`, tanpa installer, tanpa registry. Data berupa `projects.json` di sebelah exe (cadangan di `%APPDATA%\GitHubReleaseTracker` bila foldernya hanya-baca)

Aplikasi hanya membaca informasi rilis. Tidak mengunduh atau memasang apa pun.

## Keyboard dan mouse

### Menambah dan memfilter
| Aksi | Cara |
|---|---|
| Menambah proyek di kotak atas | **Enter**, atau tombol *Add project* |
| Pindah ke kotak Filter | **Ctrl+F** |
| Mengosongkan filter (teks dan status) | **Esc** di kotak Filter, atau tombol *Clear* |

### Memilih proyek
| Aksi | Cara |
|---|---|
| Pilih satu | Klik baris |
| Tambah atau lepas satu baris dari pilihan | **Ctrl+klik** |
| Pilih satu rentang | **Shift+klik**, atau **Shift+Atas / Bawah** |
| Lompat ke proyek pertama / terakhir | **Home** / **End** |
| Menggeser pilihan | **Atas / Bawah** |
| Pilih semua proyek yang tampil | **Ctrl+A** |

Hanya baris yang terlihat yang terpilih, jadi saat filter aktif, *Remove* dan *Mark as seen* hanya berlaku pada baris itu.

### Mengelola proyek
| Aksi | Cara |
|---|---|
| Ganti nama proyek terpilih | **F2**, atau klik dua kali pada nama di kolom **Project**, atau ikon pensil saat kursor di atas baris, atau klik kanan > Rename |
| Simpan / batalkan ganti nama | **Enter** / **Esc** |
| Membuka halaman rilis | Klik dua kali di kolom mana pun kecuali **Project**, atau *Open release page* |
| Menghapus proyek terpilih | **Delete**, atau *Remove* (selalu bertanya dulu) |
| Menu baris | **Klik kanan** baris: Open release page, Open repository, Mark as seen, Rename, Reset name, Include pre-releases, Remove. Klik kanan pada baris yang belum terpilih akan memilihnya dulu |

### Daftar dan jendela
| Aksi | Cara |
|---|---|
| Mengurutkan | Klik header **Project**, **Repository**, **Source**, **Published**, atau **Status**; klik lagi untuk membalik |
| Mengubah lebar kolom | Seret garis pembatas di header |
| Menggulir | Roda mouse |
| Menyalin teks status | **Klik kanan** bar status > Copy |
| Kotak teks | Klik kanan untuk Cut / Copy / Paste / Select all |
| Beralih gelap / terang | Tombol bulan / matahari di kanan bawah |

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
- Token GitHub opsional (Settings) disimpan sebagai teks biasa di `projects.json` dan tidak pernah diekspor. Pemakaian biasa tidak membutuhkannya: rilis dibaca dari feed publik GitHub. Token hanya dikirim ke `github.com`, tidak pernah ke host lain.
- Proyek di host lain dibaca lewat API publik host itu (GitLab: `/api/v4/projects/<path>/releases`, Gitea / Forgejo / Codeberg: `/api/v1/repos/<owner>/<repo>/releases`). Rilis draf dan rilis "upcoming" GitLab diabaikan. Untuk server yang tidak dikenal, aplikasi mencoba API Gitea / Forgejo dulu lalu GitLab, satu kali, saat proyek ditambahkan. Token tidak pernah dikirim ke host ini (token GitHub opsional hanya dipakai untuk `github.com`).
- Tautan yang tersimpan di data kamu hanya dibuka jika mengarah ke host proyek itu sendiri.
- Host lain: hanya repo publik (tanpa token), hanya rilis resmi (bukan tag saja), dan GitLab tidak punya penanda pre-release. Server self-hosted dengan sertifikat yang tidak dipercaya Windows ditolak (aplikasi tidak pernah mematikan pemeriksaan sertifikat). Alamat `http://` hanya dipakai kalau kamu mengetiknya.
- Versi lama (sebelum ini) tidak memahami proyek dari host lain: proyek itu dilewati saat membaca `projects.json` dan hilang saat menyimpan. Pakai versi baru di semua tempat yang berbagi file itu.

## Lisensi

MIT. Lihat [LICENSE](LICENSE).
