# Killpowa Server

A basic auth server powered by axum and sqlx (Postgres) and uses SSE (Server Sent Events) to deliver near instant updates to the client.

## App Setup

### Prerequisites

- A Postgres database credentials
- Sqlx-Cli with postgres features support. visit [Sqlx-cli crate doc](<[Sqli](https://crates.io/crates/sqlx-cli)>) on how to install

### Clone repo and cd into repo

```bash
$ git clone https://github.com/AdeThorMiwa/kpowa-server

$ cd kpower-server
```

Update config values in the `config/base.yaml` file or create an `.env` file from the `.env.sample` file and provide the appropriate config values

**NOTE:** Security sensitive configurations (like database password and jwt secret) should be set (or override) from the `.env` file

### Setup database

The `migrations/` directory contains migrations to setup the database. To setup you need to set a `DATABASE_URL ` environment variable and run the sqlx migrate command

```bash
$ export DATABASE_URL=postgres://postgres:mysecretpassword@localhost:5432/postgres

$ sqlx migrate run
```

### Start app

To start app in dev mode, run:

```bash
$ cargo run
```
