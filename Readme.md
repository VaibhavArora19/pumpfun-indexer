# 🚀 Pump.fun Indexer

A Rust-based backend indexer that connects to Solana’s Pump.fun ecosystem, storing and serving token data via RESTful APIs using Actix Web, SQLx, Redis, and Carbon.

---

## 📦 Features

- Fetch and serve token data
- Periodically update SOL price
- Store trade data in PostgreSQL (with Redis buffer)
- Actix Web REST API
- Built with async-first and multi-threaded approach for high scalability

---

## 🛠️ Requirements

- [Rust (latest stable)](https://rustup.rs)
- [PostgreSQL](https://www.postgresql.org/)
- [Redis](https://redis.io/)

## 🛠️ ENV Configuration

```env
- API_KEY="YOUR HELIUS PROFESSIONAL API KEY"
- COINGECKO_API="YOUR COINGECKO KEY"
```

## 📦 Building and running

1. Clone the repo
2. Set the env
3. In your root directory run, `docker-compose up -d`
4. Cd to db folder and run (`cargo run`)
5. Cd to indexer folder and run (`cargo run`)
6. That's it
