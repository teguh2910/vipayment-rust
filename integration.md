# Integration Guide — VIPayment Top-Up API

Panduan integrasi backend top-up untuk frontend lain (web, mobile, atau dashboard).
Semua contoh memakai gateway publik; Anda tidak perlu tahu service di belakangnya.

---

## 1. Arsitektur singkat

```
Frontend Anda ──► gateway (:8080 publik)
                    ├─► catalog-service  (:8081, internal)  katalog & harga
                    ├─► order-service    (:8082, internal)  order & pembayaran
                    └─► fulfillment      (internal)         kirim ke provider
```

Yang perlu diingat:

- **Satu base URL saja**: `https://vipsdk.ttmi.my.id` (gateway). Prefix API: `/api/v1`.
- Port 8081–8083 **internal**. Jangan diakses langsung; `/internal/*` hanya untuk antar-service.
- Harga **selalu** dari server. Client hanya mengirim `game_slug` + `product_code`.

| Prefix publik | Isi |
| --- | --- |
| `/api/v1/*` | API JSON |
| `/logos/*` | ikon grup (game/brand) |
| `/uploads/*` | bukti pembayaran yang diunggah |

---

## 2. Base URL dan CORS

Service ini dipublikasikan di:

```
https://vipsdk.ttmi.my.id
```

Base URL = origin itu, prefix API = `/api/v1`:

| Kebutuhan | URL |
| --- | --- |
| API | `https://vipsdk.ttmi.my.id/api/v1` |
| Ikon grup | `https://vipsdk.ttmi.my.id/logos/...` |
| Bukti bayar | `https://vipsdk.ttmi.my.id/uploads/...` |
| Health | `https://vipsdk.ttmi.my.id/api/v1/health` |

Cek cepat dari mana pun:

```bash
curl -s https://vipsdk.ttmi.my.id/api/v1/health | jq
```

### 2.1 Daftarkan origin web Anda

CORS dijaga **oleh gateway**, bukan oleh nginx. Origin frontend harus terdaftar di
`apps/gateway/.env`, lalu gateway di-restart:

```dotenv
# apps/gateway/.env — pisahkan dengan koma
WEB_ORIGIN=https://topup.ttmi.my.id,https://app-anda.example.com
# atau izinkan semua origin (tanpa cookie/kredensial):
# WEB_ORIGIN=*
```

```bash
docker compose up -d gateway
docker compose logs gateway | grep starting
# starting vipayment-gateway ... origins=["https://topup.ttmi.my.id"] allow_any_origin=false
```

Yang diizinkan: method `GET`, `POST`, `OPTIONS`; header `Authorization`,
`Content-Type`. Cookie/kredensial tidak dipakai, jadi jangan kirim
`credentials: "include"`.

> Origin tidak terdaftar → browser memblokir request (respons tanpa
> `Access-Control-Allow-Origin`). Ini bukan bug API. Cek dengan:
>
> ```bash
> curl -s -o /dev/null -D- -X OPTIONS https://vipsdk.ttmi.my.id/api/v1/games \
>   -H 'Origin: https://app-anda.example.com' \
>   -H 'Access-Control-Request-Method: GET' | grep -i access-control-allow-origin
> ```
>
> Ada header → origin terdaftar. Kosong → tambahkan ke `WEB_ORIGIN`.

### 2.2 Batas dari sisi nginx

Selain validasi aplikasi (bagian 10), edge juga membatasi:

| Path | Batas |
| --- | --- |
| `POST /api/v1/orders` | 12 request/menit per IP (burst 10) |
| `/api/v1/*` lain | 240 request/menit per IP (burst 60) |
| `/api/v1/health` | tanpa limit |
| Body request | 4 MB di nginx, 3 MB di gateway |
| Path di luar `/api/v1/`, `/logos/`, `/uploads/` | `404` |

Melebihi batas → **HTTP 429** dengan body bukan JSON dari aplikasi. Tangani 429
dengan backoff, jangan retry langsung.

---

## 2A. Deploy domain `vipsdk.ttmi.my.id`

Ikuti bagian ini sekali saat menyiapkan server. Anggap repo ada di
`/home/ubuntu/app/vipayment-rust` (sesuaikan bila beda).

> **Penting:** `.env` tiap service tidak masuk git (`.gitignore`). Kalau sebelumnya
> Anda hanya punya `apps/api/.env` dari arsitektur monolit, file itu **tidak**
> otomatis terbawa — buatlah `.env` baru untuk tiap service di langkah 2.

### Langkah 0 — prasyarat

- DNS `vipsdk.ttmi.my.id` → A record ke IP server (sudah aktif:
  `43.156.92.36`). Verifikasi: `getent hosts vipsdk.ttmi.my.id`.
- Port 80 & 443 terbuka di firewall/security group (untuk ACME challenge).
- Docker + Compose terpasang.

### Langkah 1 — ambil kode

```bash
cd /home/ubuntu/app
git clone https://github.com/teguh2910/vipayment-rust.git
cd vipayment-rust
git checkout refactor/microservices
```

### Langkah 2 — buat `.env` tiap service

```bash
cp apps/gateway/.env.example apps/gateway/.env
cp apps/catalog-service/.env.example apps/catalog-service/.env
cp apps/order-service/.env.example apps/order-service/.env
cp apps/fulfillment-service/.env.example apps/fulfillment-service/.env
```

Isi nilai nyata. **`INTERNAL_TOKEN` harus identik** di catalog, order, dan
fulfillment. **`ADMIN_TOKEN` harus identik** di gateway dan order.

```bash
# buat secret acak
openssl rand -hex 32   # pakai untuk INTERNAL_TOKEN
openssl rand -hex 32   # pakai untuk ADMIN_TOKEN
```

Contoh `apps/gateway/.env`:

```dotenv
BIND_ADDR=0.0.0.0:8080
WEB_ORIGIN=https://topup.ttmi.my.id
ADMIN_TOKEN=<ADMIN_TOKEN>
CATALOG_SERVICE_URL=http://catalog:8081
ORDER_SERVICE_URL=http://order:8082
```

Contoh `apps/catalog-service/.env`:

```dotenv
BIND_ADDR=0.0.0.0:8081
DATABASE_URL=sqlite://data/catalog.db
LOGO_DIR=data/logos
GAMES_CONFIG_PATH=config/games.json
LOGOS_CONFIG_PATH=config/logos.json
INTERNAL_TOKEN=<INTERNAL_TOKEN>
PROVIDER_VIPRESELLER_ENABLED=true
PROVIDER_VIPRESELLER_API_ID=<api_id>
PROVIDER_VIPRESELLER_API_KEY=<api_key>
PROVIDER_VIPRESELLER_BASE_URL=https://vip-reseller.co.id/api
PROVIDER_FORCE_IPV4=true
MARKUP_PERCENT=5
CATALOG_CATEGORIES=pulsa-reguler,pulsa-transfer,paket-internet,paket-telepon,paket-lainnya,token-pln
```

Contoh `apps/order-service/.env`:

```dotenv
BIND_ADDR=0.0.0.0:8082
DATABASE_URL=sqlite://data/orders.db
UPLOAD_DIR=data/uploads
CATALOG_SERVICE_URL=http://catalog:8081
INTERNAL_TOKEN=<INTERNAL_TOKEN>
ADMIN_TOKEN=<ADMIN_TOKEN>
PAYMENT_PROVIDER=manual
PAYMENT_EXPIRY_MINUTES=1440
MANUAL_BANK_ACCOUNTS='[{"bank":"BCA","account_no":"1234567890","account_name":"Nama Pemilik"}]'
```

Contoh `apps/fulfillment-service/.env`:

```dotenv
ORDER_SERVICE_URL=http://order:8082
INTERNAL_TOKEN=<INTERNAL_TOKEN>
POLL_INTERVAL_SECONDS=10
PROVIDER_VIPRESELLER_ENABLED=true
PROVIDER_VIPRESELLER_API_ID=<api_id>
PROVIDER_VIPRESELLER_API_KEY=<api_key>
PROVIDER_VIPRESELLER_BASE_URL=https://vip-reseller.co.id/api
```

`docker-compose.yml` mengambil `INTERNAL_TOKEN` dan `ADMIN_TOKEN` dari environment
shell, jadi ekspor juga saat menjalankan Compose:

```bash
export INTERNAL_TOKEN='<INTERNAL_TOKEN>'
export ADMIN_TOKEN='<ADMIN_TOKEN>'
```

> Semua path data (`data/catalog.db`, `data/uploads`, `data/logos`) relatif
> terhadap `/app`, dan Compose memetakan volume `catalog-data` / `order-data`
> ke sana. Jangan diganti ke path host.

### Langkah 3 — migrasi data lama (bila ada)

Lakukan **sebelum** menyalakan service baru, supaya order lama tidak hilang.

```bash
# 1. Backup dulu (wajib)
cp data/app.db data/app.db.bak
docker run --rm -v "$PWD":/w -w /w alpine cp data/app.db data/legacy.db

# 2. Split ke catalog.db + orders.db (sumber dibuka read-only)
cargo run -p vipayment-migrate   # butuh Rust di host; lihat alternatif di bawah
```

Jika Rust tidak terpasang di server, jalankan dalam container build:

```bash
docker build -f Dockerfile.service --build-arg SERVICE_BIN=vipayment-migrate -t vipayment-migrate:local .
docker run --rm -v vipayment-rust_order-data:/data -v "$PWD/data:/legacy:ro" \
  -e SOURCE_DATABASE_URL=sqlite:///legacy/app.db \
  -e ORDERS_DATABASE_URL=sqlite:///data/orders.db \
  -e CATALOG_DATABASE_URL=sqlite:///data/catalog.db \
  vipayment-migrate:local
```

Tool ini keluar non-zero bila jumlah baris tidak sama — jangan lanjut bila begitu.
Setelah migrasi, katalog akan disegarkan otomatis oleh sync (atau paksa lewat
`POST /api/v1/admin/sync-products`).

### Langkah 4 — jalankan service

```bash
docker compose up -d --build
docker compose ps
docker compose logs -f gateway catalog order fulfillment
```

Hanya gateway yang membuka port host (`127.0.0.1:8089`). catalog/order/fulfillment
tetap di jaringan internal Docker.

### Langkah 5 — terbitkan sertifikat TLS

```bash
sudo apt-get install -y certbot
sudo mkdir -p /var/www/html
sudo certbot certonly --webroot -w /var/www/html -d vipsdk.ttmi.my.id
sudo ls /etc/letsencrypt/live/vipsdk.ttmi.my.id/
```

`certbot` akan meminta email dan persetujuan ToS. Renewal otomatis sudah
dipasang oleh paket Debian (`systemctl list-timers | grep certbot`).

### Langkah 6 — pasang konfigurasi nginx

Config siap pakai ada di repo: `deploy/nginx/vipsdk.ttmi.my.id.conf`.

```bash
sudo cp deploy/nginx/vipsdk.ttmi.my.id.conf /etc/nginx/sites-available/vipsdk.ttmi.my.id.conf
sudo ln -sfn /etc/nginx/sites-available/vipsdk.ttmi.my.id.conf /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

Apa yang dilakukan config itu:

- `http://` → redirect 301 ke `https://`, dengan jalur ACME challenge.
- Meneruskan **hanya** `/api/v1/`, `/logos/`, `/uploads/` ke `127.0.0.1:8089`;
  sisanya `404` (tidak ada halaman/panel yang terbuka di domain ini).
- Rate limit `POST /api/v1/orders` (12/menit) dan `/api/v1/*` lain (240/menit).
- `/logos/` di-cache 7 hari; `/uploads/` `private, no-store` (bukti bayar privat).
- **Tidak** menyentuh header CORS — itu milik gateway.

### Langkah 7 — verifikasi

```bash
# 1. TLS + redirect
curl -sI http://vipsdk.ttmi.my.id/api/v1/health | head -3        # 301
curl -s https://vipsdk.ttmi.my.id/api/v1/health | jq             # ok: true

# 2. katalog
curl -s https://vipsdk.ttmi.my.id/api/v1/games | jq '.games | length'

# 3. CORS untuk origin web Anda
curl -s -o /dev/null -D- -X OPTIONS https://vipsdk.ttmi.my.id/api/v1/games \
  -H 'Origin: https://topup.ttmi.my.id' \
  -H 'Access-Control-Request-Method: GET' | grep -i access-control-allow-origin

# 4. admin auth
curl -s -o /dev/null -w '%{http_code}\n' https://vipsdk.ttmi.my.id/api/v1/admin/orders   # 401
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" \
  https://vipsdk.ttmi.my.id/api/v1/admin/profile | jq

# 5. path lain tertutup
curl -s -o /dev/null -w '%{http_code}\n' https://vipsdk.ttmi.my.id/            # 404
```

Lalu uji satu checkout penuh dari web Anda (bagian 6), dan pastikan
`pending_payment → paid → processing → success`.

### Langkah 8 — pakai dari web lain

Cukup dua hal di sisi frontend:

```js
const API_BASE = "https://vipsdk.ttmi.my.id";
const res = await fetch(`${API_BASE}/api/v1/games`);
```

dan daftarkan origin web tersebut di `WEB_ORIGIN` (langkah 2.1). Jika web lain
berada di server yang sama dengan nginx ini, Anda juga bisa mem-proxy dari
domain web tersebut agar tidak perlu CORS sama sekali:

```nginx
# di server block domain web Anda
location /api/ {
    proxy_pass http://127.0.0.1:8089;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Dengan cara itu frontend memanggil `/api/v1/...` di domainnya sendiri.

### Troubleshooting deploy

| Gejala | Penyebab / solusi |
| --- | --- |
| `nginx -t` gagal: unknown directive `http2` | nginx < 1.25.1 — config di repo sudah memakai `listen 443 ssl http2;` |
| Certbot gagal `unauthorized` | port 80 tidak terbuka, atau DNS belum mengarah ke server ini |
| `502 Bad Gateway` | gateway mati: `docker compose ps`, `docker compose logs gateway` |
| Health OK tapi `/games` kosong | sync belum jalan / IP server belum di-allowlist provider — cek `last_sync_error` |
| Browser: CORS error | origin belum di `WEB_ORIGIN`; restart gateway |
| `413 Request Entity Too Large` | upload > 4 MB (nginx) atau > 3 MB (gateway) |
| `429` | kena rate limit edge atau batas 8 order/10 menit per kontak |
| Logo 404 | `/logos/` tidak diteruskan proxy |
| Data lama hilang | migrasi (langkah 3) dilewati atau `DATABASE_URL` berbeda |

---

## 3. Autentikasi

| Golongan | Auth | Dipakai siapa |
| --- | --- | --- |
| `/api/v1/*` publik | Tanpa auth | storefront / pengunjung |
| `/api/v1/admin/*` | `Authorization: Bearer <ADMIN_TOKEN>` | dashboard admin (server-side) |

`ADMIN_TOKEN` **tidak boleh** ada di bundle browser. Panggil route admin dari
server Anda sendiri atau dashboard yang sudah terautentikasi.

```http
GET /api/v1/admin/orders HTTP/1.1
Authorization: Bearer <ADMIN_TOKEN>
```

Tanpa token → `401`:

```json
{ "error": "unauthorized", "message": "unauthorized" }
```

### 3.1 Cara menguji auth

Ada dua jenis auth yang berbeda dan sering tertukar:

| Jenis | Header | Untuk | Tidak berlaku di |
| --- | --- | --- | --- |
| Admin | `Authorization: Bearer <ADMIN_TOKEN>` | `/api/v1/admin/*` | endpoint publik |
| Internal | `x-internal-token: <INTERNAL_TOKEN>` | `/internal/*` (antar-service) | gateway publik |

`/internal/*` **tidak dirutekan** oleh gateway, jadi dari luar hasilnya `404`.
Header internal hanya relevan bila Anda menembak service langsung (mis. saat
debug di server).

Uji manual (ganti `$ADMIN_TOKEN` dan `$API`):

```bash
API=https://vipsdk.ttmi.my.id

# 1. publik: harus 200 tanpa header apa pun
curl -s -o /dev/null -w '%{http_code}\n' "$API/api/v1/health"

# 2. admin tanpa token: harus 401
curl -s -o /dev/null -w '%{http_code}\n' "$API/api/v1/admin/orders"

# 3. token salah: harus 401
curl -s -o /dev/null -w '%{http_code}\n' \
  -H 'Authorization: Bearer salah' "$API/api/v1/admin/orders"

# 4. token benar: harus 200
curl -s -o /dev/null -w '%{http_code}\n' \
  -H "Authorization: Bearer $ADMIN_TOKEN" "$API/api/v1/admin/orders"

# 5. token tanpa prefix "Bearer " dan huruf kecil: harus 401
curl -s -o /dev/null -w '%{http_code}\n' \
  -H "Authorization: $ADMIN_TOKEN" "$API/api/v1/admin/orders"
curl -s -o /dev/null -w '%{http_code}\n' \
  -H "Authorization: bearer $ADMIN_TOKEN" "$API/api/v1/admin/orders"

# 6. internal lewat gateway: harus 404 (tidak diekspos)
curl -s -o /dev/null -w '%{http_code}\n' -X POST "$API/internal/orders/claim"
```

Semua sekaligus, dengan exit code (cocok untuk CI / cek setelah deploy):

```bash
./scripts/test-auth.sh https://vipsdk.ttmi.my.id "$ADMIN_TOKEN"
```

Verifikasi `/internal/*` di service langsung (hanya dari server):

```bash
# tanpa header → 401, header salah → 401, header benar → 204 (tidak ada order)
curl -s -o /dev/null -w '%{http_code}\n' -X POST http://127.0.0.1:8082/internal/orders/claim
curl -s -o /dev/null -w '%{http_code}\n' -X POST \
  -H "x-internal-token: $INTERNAL_TOKEN" http://127.0.0.1:8082/internal/orders/claim
```

Catatan penting:

- Prefix harus persis `Bearer ` (huruf besar, satu spasi). `bearer `, token
  telanjang, atau spasi ganda ditolak `401`.
- Detail kecil memang disengaja: **tidak** ada informasi apakah token salah atau
  tidak ada; keduanya `401` dengan body identik.
- `ADMIN_TOKEN` divalidasi **dua kali** — di gateway dan di order service. Jadi
  token gateway dan order harus sama, kalau tidak admin akan `401` walau token
  benar di gateway.
- `INTERNAL_TOKEN` harus sama di catalog, order, dan fulfillment. Kalau berbeda,
  checkout gagal dengan `502 provider_error` karena order service tidak bisa
  memanggil catalog.
- Jangan pernah menaruh kedua token ini di bundle frontend.

---

## 4. Konvensi umum

- **Format uang**: integer rupiah (`19950`). `amount_label` disediakan untuk tampilan
  (`"19.950"`).
- **Waktu**: string RFC 3339 UTC (`"2026-11-13T14:53:12.345Z"`).
- **Kontak**: nomor WhatsApp (min. 8 digit) atau email valid. Disimpan sebagai yang
  dikirim; respons menampilkannya tersamar (`0812****890`).
- **Body limit**: 3 MB (gateway). Bukti bayar maksimal 2 MB.

### Format error

Semua error memakai bentuk yang sama:

```json
{ "error": "not_found", "message": "produk tidak ditemukan" }
```

| `error` | HTTP | Arti untuk integrasi |
| --- | --- | --- |
| `bad_request` | 400 | Input tidak valid (field kosong/terlalu panjang/format kontak) |
| `unauthorized` | 401 | `ADMIN_TOKEN` salah atau tidak ada |
| `not_found` | 404 | Grup/produk/invoice tidak ada (atau disembunyikan `CATALOG_CATEGORIES`) |
| `conflict` | 409 | Produk tidak tersedia, status order tidak mengizinkan aksi |
| `too_many_requests` | 429 | Terlalu banyak order dari kontak yang sama |
| `provider_error` | 502 | Provider gagal/ tidak bisa dihubungi (order tetap aman) |
| `internal_error` | 500 | Kesalahan internal; coba lagi nanti |

Tampilkan `message` ke pengguna; jangan parsing berdasarkan teks.

---

## 5. Katalog

### 5.1 Health

```http
GET /api/v1/health
```

```json
{
  "ok": true,
  "provider_enabled": true,
  "payment_provider": "manual",
  "last_sync_at": "2026-11-13T14:52:49.532Z",
  "last_sync_error": null,
  "logos_cached": 56,
  "catalog_service_ok": true,
  "order_service_ok": true
}
```

`last_sync_error` berisi pesan seperti `IP ... tidak diizinkan` bila provider menolak
IP server. `catalog_service_ok`/`order_service_ok` = `false` berarti satu service down.

### 5.2 Daftar grup

```http
GET /api/v1/games
```

```json
{
  "games": [
    {
      "slug": "mobile-legends",
      "name": "Mobile Legends",
      "icon_url": "/logos/mobilelegends-com.png",
      "requires_zone": true,
      "user_id_label": "User ID",
      "zone_label": "Zone ID",
      "kind": "game",
      "category": null,
      "category_label": null,
      "product_count": 12,
      "min_price": 3990,
      "min_price_label": "3.990"
    },
    {
      "slug": "pp-token-pln-pln",
      "name": "Token PLN",
      "icon_url": null,
      "requires_zone": false,
      "user_id_label": "ID Pelanggan / No. Meter",
      "zone_label": "Zone ID",
      "kind": "prepaid",
      "category": "token-pln",
      "category_label": "Token PLN",
      "product_count": 8,
      "min_price": 21000,
      "min_price_label": "21.000"
    }
  ]
}
```

> **Slug tidak selalu sama dengan nama.** Grup tanpa override di
> `config/games.json` mendapat slug hasil generate (`g-<nama>`, misalnya
> `g-mobile-legends`), sedangkan yang punya override memakai slug tetap
> (misalnya `mobile-legends`). **Selalu ambil `slug` dari respons `/games`**,
> jangan menyusunnya sendiri dari nama tampilan.

Gunakan `kind` untuk perilaku form:

| `kind` | Kirim `zone`? | Label input utama |
| --- | --- | --- |
| `game` | Hanya jika `requires_zone` | `user_id_label` |
| `app` | Tidak | `user_id_label` |
| `prepaid` | Tidak (diabaikan server) | `user_id_label` (mis. nomor HP / ID meter) |

`icon_url` null → tampilkan tile inisial. Jika berisi path relatif, gabungkan dengan base URL.

### 5.3 Produk satu grup

```http
GET /api/v1/games/mobile-legends/products
```

```json
{
  "game": {
    "slug": "mobile-legends",
    "name": "Mobile Legends",
    "icon_url": "/logos/mobilelegends-com.png",
    "requires_zone": true,
    "user_id_label": "User ID",
    "zone_label": "Zone ID",
    "kind": "game",
    "category": null,
    "category_label": null
  },
  "products": [
    { "code": "ML86", "name": "86 Diamonds", "sell_price": 19950, "price_label": "19.950" }
  ]
}
```

Produk yang `available = 0` tidak muncul. `404` bila grup tidak ada atau disembunyikan
oleh allowlist `CATALOG_CATEGORIES`.

---

## 6. Alur checkout (paling penting)

```
pilih produk ──► POST /orders ──► bayar + upload bukti ──► admin approve
                                        │                       │
                                   GET /orders/{invoice}  ◄──────┘
                                        │  (polling)
                            paid ─► submitting ─► processing ─► success
```

### 6.1 Buat order

```http
POST /api/v1/orders
Content-Type: application/json
```

```json
{
  "game_slug": "mobile-legends",
  "product_code": "ML86",
  "user_id": "12345678",
  "zone": "1234",
  "contact": "081234567890",
  "idempotency_key": "cart-8f3a9c"
}
```

| Field | Wajib | Batas | Catatan |
| --- | --- | --- | --- |
| `game_slug` | ya | ≤ 64 | dari `/games` |
| `product_code` | ya | ≤ 64 | dari `/games/{slug}/products` |
| `user_id` | ya | ≤ 64 | ID game / nomor HP / ID meter |
| `zone` | bila `requires_zone` | ≤ 32 | diabaikan untuk non-`game` |
| `contact` | ya | ≤ 64 | WhatsApp (≥ 8 digit) atau email |
| `idempotency_key` | tidak | ≤ 128 | **sangat disarankan** (lihat 6.2) |

Respons `201 Created` (bentuk sama dengan `GET /orders/{invoice}`):

```json
{
  "invoice": "INV-261113-VE2SBQ1E",
  "status": "pending_payment",
  "amount": 19950,
  "amount_label": "19.950",
  "created_at": "2026-11-13T14:53:12.345Z",
  "paid_at": null,
  "completed_at": null,
  "game": {
    "slug": "g-mobile-legends",
    "name": "Mobile Legends",
    "icon_url": "/logos/mobilelegends-com.png",
    "requires_zone": true,
    "kind": "game",
    "category": null,
    "category_label": null
  },
  "product": { "code": "ML86", "name": "86 Diamonds" },
  "account": {
    "user_id": "12345678",
    "zone": "1234",
    "user_id_label": "User ID",
    "zone_label": "Zone ID"
  },
  "contact": "0812****890",
  "provider": { "trxid": null, "status": null, "sn": null, "failure_reason": null },
  "payment": {
    "status": "pending",
    "status_kind": "pending",
    "provider": "manual",
    "method": "qris",
    "amount": 19950,
    "proof_url": null,
    "proof_uploaded_at": null,
    "expires_at": "2026-11-14T14:53:12.345Z",
    "reject_reason": null,
    "instructions": {
      "provider": "manual",
      "method": "qris",
      "amount": 19950,
      "qris_image_url": "https://.../qris.png",
      "bank_accounts": [
        { "bank": "BCA", "account_no": "1234567890", "account_name": "Nama Pemilik" }
      ],
      "instructions": ["Bayar tepat Rp 19.950 untuk invoice INV-...", "..."],
      "expires_at": "2026-11-14T14:53:12.345Z"
    }
  },
  "events": [
    { "from": null, "to": "pending_payment", "note": "order dibuat, menunggu pembayaran", "at": "2026-11-13T14:53:12.345Z" }
  ]
}
```

Catatan penting:

- `payment.instructions` hanya ada selama `status = pending_payment` dan pembayaran
  masih `pending` / `awaiting_review` / `rejected`. Di luar itu `null`.
- `amount` adalah harga final dari server. Jangan menampilkan harga dari client.
- Mode `simulation` (untuk testing) mengonfirmasi pembayaran otomatis dalam beberapa detik.
  Pada mode ini `payment.method = "simulation"`, `qris_image_url = null`, dan
  `bank_accounts = []` — tampilkan `instructions` saja, jangan render QR/bank kosong.

### 6.2 Idempotency (mencegah order ganda)

Kirim `idempotency_key` unik per percobaan checkout (mis. hash isi keranjang). Request
ulang dengan key yang sama mengembalikan **invoice yang sama**, bukan order baru.

```js
// key stabil per keranjang, bukan per klik
const idempotency_key = `cart-${cart.id}-${cart.updatedAt}`;
```

Tanpa key ini, double-click atau retry jaringan dapat membuat dua invoice.

### 6.3 Unggah bukti pembayaran

```http
POST /api/v1/orders/INV-261113-VE2SBQ1E/payment-proof
Content-Type: multipart/form-data
```

```bash
curl -X POST "$API/api/v1/orders/INV-261113-VE2SBQ1E/payment-proof" \
  -F "file=@bukti.jpg"
```

Aturan:

- Nama field **`file`**.
- Maks **2 MB**, harus JPG/PNG/WEBP (dicek dari magic bytes, bukan sekadar ekstensi).
- Hanya bisa saat order `pending_payment` dan pembayaran `pending`/`awaiting_review`/`rejected`
  dan belum lewat `expires_at`. Selain itu `409`.

Respons:

```json
{
  "ok": true,
  "invoice": "INV-261113-VE2SBQ1E",
  "proof_url": "/uploads/0b0c...e1.jpg"
}
```

`proof_url` relatif terhadap base URL — gabungkan untuk ditampilkan.

### 6.4 Pantau status

```http
GET /api/v1/orders/{invoice}
```

Bentuk respons sama dengan 6.1. Pakai `status` untuk UI, `events` untuk timeline.

Rekomendasi polling (tidak ada webhook):

- Saat `pending_payment`: setiap 5–10 detik (cukup untuk mode simulasi).
- Saat `processing`: setiap 10–20 detik.
- Berhenti pada status final: `success`, `failed`, `refunded`, `expired`, `needs_attention`.

### 6.5 Cari order berdasarkan kontak

```http
GET /api/v1/orders/lookup?contact=081234567890
```

Menerima variasi nomor (`0812...`, `62812...`, `+62 812-...`) dan email
(case-insensitive). Maksimal 20 hasil terbaru.

```json
{
  "orders": [
    {
      "invoice": "INV-261113-VE2SBQ1E",
      "product_name": "86 Diamonds",
      "amount": 19950,
      "amount_label": "19.950",
      "status": "success",
      "created_at": "2026-11-13T14:53:12.345Z"
    }
  ]
}
```

---

## 7. Status

### Order (`status`)

| Status | Arti | Final? |
| --- | --- | --- |
| `pending_payment` | menunggu pembayaran | tidak |
| `paid` | pembayaran diterima, antre dikirim | tidak |
| `submitting` | sedang dikirim ke provider (ada lease) | tidak |
| `processing` | provider memproses | tidak |
| `success` | selesai, `provider.sn` berisi serial number | **ya** |
| `failed` | gagal (umumnya langsung jadi `refund_pending`) | tidak |
| `refund_pending` | perlu refund manual | tidak |
| `refunded` | refund selesai | **ya** |
| `expired` | lewat batas waktu | **ya** |
| `needs_attention` | gagal kirim berulang, perlu admin | **ya** (sampai admin retry) |

Alur normal: `pending_payment → paid → submitting → processing → success`.

`submitting` bisa terlihat sesaat; jangan anggap itu status final.

### Pembayaran (`payment.status`)

| Status | Arti |
| --- | --- |
| `pending` | belum dibayar |
| `awaiting_review` | bukti terunggah, menunggu verifikasi admin |
| `paid` | lunas (manual approval atau simulasi) |
| `rejected` | bukti ditolak (`reject_reason` terisi), boleh unggah ulang |
| `expired` | batas waktu habis |

---

## 8. Endpoint admin

Semua butuh `Authorization: Bearer <ADMIN_TOKEN>`.

| Method | Path | Query/Body | Respons |
| --- | --- | --- | --- |
| GET | `/api/v1/admin/payments` | `?status=awaiting_review&q=` | `{ payments: [...] }` |
| GET | `/api/v1/admin/orders` | `?status=all&q=` | `{ orders: [...] }` |
| POST | `/api/v1/admin/payments/{invoice}/approve` | — | order view |
| POST | `/api/v1/admin/payments/{invoice}/reject` | `{"reason":"..."}` | order view |
| POST | `/api/v1/admin/orders/{invoice}/retry` | — | order view |
| POST | `/api/v1/admin/orders/{invoice}/refunded` | — | order view |
| POST | `/api/v1/admin/sync-products` | — | `{groups, products, logos, errors}` |
| GET | `/api/v1/admin/profile` | — | saldo provider + margin aktif |

Detail perilaku:

- `GET /admin/payments` default `status=awaiting_review` (antrean verifikasi). Pakai
  `status=all` untuk semua, atau `paid`/`rejected`/`expired`.
- `q` melakukan pencarian `LIKE` di:
  - `/admin/payments` → invoice, nama produk, user id, kontak, `provider_trxid`;
  - `/admin/orders` → semua di atas **plus** `provider_sn`.

  Maks 200 baris terbaru, urut terbaru dulu.
- `POST /admin/orders/{invoice}/retry` hanya untuk `needs_attention`, `failed`,
  `refund_pending` → mengembalikan ke `paid`.
- `POST /admin/orders/{invoice}/refunded` hanya untuk `refund_pending`.
- Approve bersifat idempotent: approve dua kali tidak menggandakan apa pun.

Contoh item `admin/payments`:

```json
{
  "invoice": "INV-261113-VE2SBQ1E",
  "game_slug": "g-mobile-legends",
  "product_name": "86 Diamonds",
  "user_id": "12345678",
  "zone": "1234",
  "contact": "081234567890",
  "amount": 19950,
  "amount_label": "19.950",
  "order_status": "pending_payment",
  "provider_trxid": null,
  "provider_status": null,
  "payment_status": "awaiting_review",
  "method": "qris",
  "proof_url": "/uploads/0b0c...e1.jpg",
  "proof_uploaded_at": "2026-11-13T14:55:00.000Z",
  "expires_at": "2026-11-14T14:53:12.345Z",
  "reject_reason": null,
  "created_at": "2026-11-13T14:53:12.345Z"
}
```

Respons `sync-products`:

```json
{ "groups": 98, "products": 1243, "logos": 56, "errors": [] }
```

---

## 9. Contoh implementasi

### TypeScript — client minimal

```ts
const API_BASE = "https://vipsdk.ttmi.my.id";

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public code?: string,
  ) {
    super(message);
  }
}

async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (typeof init.body === "string" && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const res = await fetch(`${API_BASE}/api/v1${path}`, { ...init, headers });
  const text = await res.text();
  const payload = text ? JSON.parse(text) : null;

  if (!res.ok) {
    throw new ApiError(
      payload?.message ?? "Permintaan gagal",
      res.status,
      payload?.error,
    );
  }
  return payload as T;
}

export const listGames = () => api<{ games: Game[] }>("/games");

export const listProducts = (slug: string) =>
  api<{ game: Game; products: Product[] }>(
    `/games/${encodeURIComponent(slug)}/products`,
  );

export const createOrder = (input: CreateOrderInput) =>
  api<OrderView>("/orders", { method: "POST", body: JSON.stringify(input) });

export const getOrder = (invoice: string) =>
  api<OrderView>(`/orders/${encodeURIComponent(invoice)}`);

export const lookupOrders = (contact: string) =>
  api<{ orders: LookupOrder[] }>(
    `/orders/lookup?contact=${encodeURIComponent(contact)}`,
  );

export async function uploadProof(invoice: string, file: File) {
  const form = new FormData();
  form.append("file", file);

  const res = await fetch(
    `${API_BASE}/api/v1/orders/${encodeURIComponent(invoice)}/payment-proof`,
    { method: "POST", body: form },
  );
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new ApiError(body?.message ?? "Upload gagal", res.status, body?.error);
  }
  return res.json() as Promise<{ ok: true; invoice: string; proof_url: string }>;
}

/** URL absolut untuk file dari API (icon atau bukti bayar). */
export const fileUrl = (path: string | null) =>
  path ? (path.startsWith("http") ? path : `${API_BASE}${path}`) : null;
```

### React — form checkout penuh

```tsx
function useCheckout(slug: string, code: string) {
  const [order, setOrder] = useState<OrderView | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function submit(input: {
    userId: string;
    zone?: string;
    contact: string;
  }) {
    setError(null);
    try {
      const created = await createOrder({
        game_slug: slug,
        product_code: code,
        user_id: input.userId,
        zone: input.zone,
        contact: input.contact,
        // stabil per percobaan checkout, bukan per klik
        idempotency_key: `cart-${slug}-${code}-${input.userId}`,
      });
      setOrder(created);
      return created;
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Terjadi kesalahan");
    }
  }

  return { order, setOrder, error, submit };
}

// polling sampai status final
function useOrderPolling(invoice: string | null) {
  const [order, setOrder] = useState<OrderView | null>(null);
  const FINAL = new Set(["success", "failed", "refunded", "expired", "needs_attention"]);

  useEffect(() => {
    if (!invoice) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;

    const tick = async () => {
      if (!alive) return;
      try {
        const next = await getOrder(invoice);
        if (!alive) return;
        setOrder(next);
        if (FINAL.has(next.status)) return;
        timer = setTimeout(
          tick,
          next.status === "processing" ? 15_000 : 7_000,
        );
      } catch {
        timer = setTimeout(tick, 15_000);
      }
    };

    tick();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [invoice]);

  return order;
}
```

### curl — uji cepat

```bash
API=https://vipsdk.ttmi.my.id

# katalog
curl -s "$API/api/v1/health" | jq
curl -s "$API/api/v1/games" | jq '.games[0]'
curl -s "$API/api/v1/games/mobile-legends/products" | jq

# buat order
INV=$(curl -s -X POST "$API/api/v1/orders" \
  -H 'content-type: application/json' \
  -d '{"game_slug":"mobile-legends","product_code":"ML86","user_id":"12345678","zone":"1234","contact":"081234567890","idempotency_key":"demo-1"}' \
  | jq -r .invoice)
echo "$INV"

# pantau
curl -s "$API/api/v1/orders/$INV" | jq '.status, .provider'

# unggah bukti (mode manual)
curl -s -X POST "$API/api/v1/orders/$INV/payment-proof" -F "file=@bukti.jpg" | jq

# admin
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" "$API/api/v1/admin/payments" | jq
curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
  "$API/api/v1/admin/payments/$INV/approve" | jq '.status'
```

---

## 10. Batas & aturan validasi

| Aturan | Nilai |
| --- | --- |
| Order per kontak | maks **8** per 10 menit (lebih → `429`) |
| Nominal order | ≤ `MAX_ORDER_AMOUNT` (default 10.000.000) |
| Panjang field | slug/code/user_id/contact ≤ 64, zone ≤ 32, idempotency ≤ 128 |
| Body request | ≤ 3 MB |
| Bukti bayar | ≤ 2 MB, JPG/PNG/WEBP |
| Daftar admin | maks 200 baris terbaru |
| Lookup | maks 20 hasil |

Rate limit dihitung per `contact`, bukan per IP. Jika user checkout berulang,
gunakan `idempotency_key` yang sama agar tidak membuat invoice baru.

---

## 11. Catatan keamanan

- Jangan taruh `ADMIN_TOKEN` atau `INTERNAL_TOKEN` di kode frontend.
- Jangan percaya harga dari client; server selalu memakai harga dari database.
- `proof_url` berisi bukti pembayaran. Jangan tampilkan publik; hanya di halaman
  invoice pemilik dan dashboard admin.
- `contact` pada respons sudah tersamar. Simpan alamat lengkap hanya jika perlu.
- API key provider tidak pernah keluar dari server.

---

## 12. Troubleshooting

| Gejala | Penyebab umum |
| --- | --- |
| Browser: `CORS policy` | Origin belum ada di `WEB_ORIGIN`; restart gateway |
| `Mixed Content` di HTTPS | Base URL masih `http://`; pakai HTTPS |
| `404 not_found` saat order | `game_slug` salah (lihat `/games`), atau kategori disembunyikan `CATALOG_CATEGORIES` |
| `409 produk sedang tidak tersedia` | Produk `available = 0`; minta user pilih produk lain |
| `429 too_many_requests` | > 8 order/10 menit dari kontak sama |
| Order macet di `pending_payment` | Belum bayar / bukti belum diunggah / belum di-approve admin |
| `needs_attention` | Pengiriman ke provider gagal berulang; admin perlu `retry` |
| Invoice dobel | `idempotency_key` tidak dikirim atau berubah tiap klik |
| Ikon 404 | `/logos/` tidak diteruskan reverse proxy ke gateway |

Jika memakai reverse proxy (nginx/Caddy), teruskan **ketiga** prefix berikut ke gateway:

```
/api/v1/   → gateway
/uploads/  → gateway
/logos/    → gateway
```

---

## 13. Ringkasan endpoint

| Method | Path | Auth | Keterangan |
| --- | --- | --- | --- |
| GET | `/api/v1/health` | — | status agregat |
| GET | `/api/v1/games` | — | daftar grup |
| GET | `/api/v1/games/{slug}/products` | — | produk + harga |
| POST | `/api/v1/orders` | — | buat order |
| GET | `/api/v1/orders/{invoice}` | — | detail + instruksi bayar + timeline |
| GET | `/api/v1/orders/lookup?contact=` | — | cari order |
| POST | `/api/v1/orders/{invoice}/payment-proof` | — | unggah bukti |
| GET | `/api/v1/admin/payments` | Bearer | antrean pembayaran |
| POST | `/api/v1/admin/payments/{invoice}/approve` | Bearer | setujui |
| POST | `/api/v1/admin/payments/{invoice}/reject` | Bearer | tolak (`{reason}`) |
| GET | `/api/v1/admin/orders` | Bearer | daftar order |
| POST | `/api/v1/admin/orders/{invoice}/retry` | Bearer | kirim ulang |
| POST | `/api/v1/admin/orders/{invoice}/refunded` | Bearer | tandai refund |
| POST | `/api/v1/admin/sync-products` | Bearer | sync katalog |
| GET | `/api/v1/admin/profile` | Bearer | saldo & margin |
