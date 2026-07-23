CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL UNIQUE,
  telegram_handle TEXT,
  usdt_trc20 TEXT,
  usdt_bep20 TEXT,
  usdt_erc20 TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE login_tokens (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  expires_at TIMESTAMPTZ NOT NULL,
  used_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE orders (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  customer_id UUID NOT NULL REFERENCES users(id),
  courier_id UUID REFERENCES users(id),
  fiat_currency TEXT NOT NULL CHECK (fiat_currency IN ('IDR','AED','EUR','GBP','RUB','INR','USD','CAD')),
  fiat_amount NUMERIC(18,2) NOT NULL CHECK (fiat_amount > 0),
  usdt_amount NUMERIC(18,6) NOT NULL CHECK (usdt_amount > 0),
  address_text TEXT NOT NULL,
  lat DOUBLE PRECISION NOT NULL,
  lng DOUBLE PRECISION NOT NULL,
  status TEXT NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','ACCEPTED','AWAITING_PAYMENT','PAID','COMPLETED','CANCELLED')),
  payment_network TEXT CHECK (payment_network IN ('trc20','bep20','erc20')),
  payment_txid TEXT UNIQUE,
  payment_requested_at TIMESTAMPTZ,
  paid_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  accepted_at TIMESTAMPTZ,
  completed_at TIMESTAMPTZ,
  cancelled_at TIMESTAMPTZ
);

CREATE INDEX orders_status_idx ON orders(status);
CREATE INDEX orders_customer_idx ON orders(customer_id);
CREATE INDEX orders_courier_idx ON orders(courier_id);
