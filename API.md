# VIPayment API Service Documentation

Backend service tunggal (`vipayment-server`) berbasis Rust (Axum) yang beroperasi sebagai **Direct / Stateless API Gateway** untuk query produk dan pemesanan layanan ke VIP Reseller API secara real-time.

---

## 1. Konfigurasi Environment

File `.env` di direktori `apps/server/.env` atau environment sistem:

```dotenv
BIND_ADDR=0.0.0.0:8080
VIPAYMENT_API_ID=your_api_id
VIPAYMENT_API_KEY=your_api_key
VIPAYMENT_BASE_URL=https://vip-reseller.co.id/api
CORS_ORIGIN=*
```

---

## 2. Menjalankan Service

### Menggunakan Cargo
```bash
cargo run -p vipayment-server
```

### Menggunakan Docker Compose
```bash
docker compose up -d --build
```
Service akan berjalan di port `8080` (dimapping ke `127.0.0.1:8089` pada `docker-compose.yml`).

---

## 3. Daftar Endpoint

Prefix utama: `/api/v1`

### 3.1 Health Check
Mengecek status service dan sisa saldo akun VIP Reseller.

- **URL**: `GET /api/v1/health` (atau `GET /health`)
- **Contoh Request**:
  ```bash
  curl -s http://127.0.0.1:8080/api/v1/health | jq
  ```
- **Contoh Response**:
  ```json
  {
    "status": "ok",
    "provider": {
      "connected": true,
      "username": "teguh_reseller",
      "balance": 500000
    }
  }
  ```

---

### 3.2 Get Products (Katalog / Layanan)

#### A. Prepaid Services (Pulsa, Paket Data, Token PLN, dll.)
- **URL**: `GET /api/v1/products/prepaid`
- **Query Parameters (Opsional)**:
  - `filter_type` atau `type`: Tipe filter (contoh: `type`, `brand`).
  - `filter_value` atau `brand`: Nilai filter (contoh: `pulsa-reguler`, `telkomsel`).
- **Contoh Request**:
  ```bash
  # Semua produk prepaid
  curl -s http://127.0.0.1:8080/api/v1/products/prepaid | jq

  # Filter berdasarkan brand Telkomsel
  curl -s "http://127.0.0.1:8080/api/v1/products/prepaid?filter_type=brand&filter_value=telkomsel" | jq
  ```

#### B. Game & Streaming Services (Mobile Legends, Free Fire, dll.)
- **URL**: `GET /api/v1/products/game`
- **Query Parameters (Opsional)**:
  - `filter_game` atau `game`: Nama game (contoh: `Mobile Legends`, `Free Fire`).
  - `filter_status` atau `status`: Status layanan (contoh: `available`, `empty`).
- **Contoh Request**:
  ```bash
  curl -s "http://127.0.0.1:8080/api/v1/products/game?filter_game=Mobile%20Legends" | jq
  ```

#### C. Unified Products Endpoint
- **URL**: `GET /api/v1/products`
- **Query Parameters**:
  - `kind`: `prepaid` (default) atau `game`.
  - Filter sama seperti di atas sesuai dengan `kind`.
- **Contoh Request**:
  ```bash
  curl -s "http://127.0.0.1:8080/api/v1/products?kind=game&game=Free%20Fire" | jq
  ```

---

### 3.3 Orders (Transaksi & Status)

#### A. Order Prepaid
- **URL**: `POST /api/v1/orders/prepaid`
- **Headers**: `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "service_code": "PLN50",
    "data_no": "1234567890"
  }
  ```
  *(Catatan: Anda juga bisa menggunakan key `"service"` sebagai alias dari `"service_code"`)*
- **Contoh Request**:
  ```bash
  curl -s -X POST http://127.0.0.1:8080/api/v1/orders/prepaid \
    -H "Content-Type: application/json" \
    -d '{"service_code":"PLN50","data_no":"1234567890"}' | jq
  ```
- **Contoh Response**:
  ```json
  {
    "result": true,
    "message": "Pesanan berhasil dibuat",
    "data": {
      "trxid": "VIP12345678",
      "service": "PLN50",
      "data_no": "1234567890",
      "status": "waiting",
      "note": "Sedang diproses"
    }
  }
  ```

#### B. Order Game
- **URL**: `POST /api/v1/orders/game`
- **Headers**: `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "service_code": "ML86",
    "data_no": "12345678",
    "data_zone": "1234"
  }
  ```
  *(Catatan: `data_zone` / `zone` bersifat opsional tergantung kebutuhan game)*
- **Contoh Request**:
  ```bash
  curl -s -X POST http://127.0.0.1:8080/api/v1/orders/game \
    -H "Content-Type: application/json" \
    -d '{"service_code":"ML86","data_no":"12345678","data_zone":"1234"}' | jq
  ```

#### C. Unified Order Endpoint
- **URL**: `POST /api/v1/orders`
- **Headers**: `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "kind": "game",
    "service_code": "ML86",
    "data_no": "12345678",
    "data_zone": "1234"
  }
  ```

#### D. Cek Status Order
- **URL**: `GET /api/v1/orders/{trxid}`
- **Query Parameters (Opsional)**:
  - `kind`: `prepaid` (default) atau `game`.
  - `limit`: Jumlah data status (opsional).
- **Contoh Request**:
  ```bash
  curl -s "http://127.0.0.1:8080/api/v1/orders/VIP12345678" | jq
  curl -s "http://127.0.0.1:8080/api/v1/orders/VIP-GAME-99?kind=game" | jq
  ```
- **Contoh Response**:
  ```json
  {
    "result": true,
    "message": "Data didapatkan",
    "data": [
      {
        "trxid": "VIP12345678",
        "service": "PLN50",
        "status": "success",
        "sn": "1234-5678-9012-3456-7890",
        "note": "Transaksi Sukses"
      }
    ]
  }
  ```
