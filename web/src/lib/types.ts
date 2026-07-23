export type Currency = "IDR" | "AED" | "EUR" | "GBP" | "RUB" | "INR" | "USD" | "CAD";
export const CURRENCIES: Currency[] = ["IDR", "AED", "EUR", "GBP", "RUB", "INR", "USD", "CAD"];

export type OrderStatus = "OPEN" | "ACCEPTED" | "AWAITING_PAYMENT" | "PAID" | "COMPLETED" | "CANCELLED";
export type Network = "trc20" | "bep20" | "erc20";

export interface User {
  id: string;
  email: string;
  telegram_handle: string | null;
  usdt_trc20: string | null;
  usdt_bep20: string | null;
  usdt_erc20: string | null;
}

export interface Order {
  id: string;
  customer_id: string;
  courier_id: string | null;
  fiat_currency: Currency;
  fiat_amount: string;
  usdt_amount: string;
  address_text: string;
  lat: number;
  lng: number;
  status: OrderStatus;
  payment_network: Network | null;
  payment_txid: string | null;
  payment_requested_at: string | null;
  paid_at: string | null;
  created_at: string;
  accepted_at: string | null;
  completed_at: string | null;
  cancelled_at: string | null;
}

export interface OrderDetail extends Order {
  is_customer: boolean;
  is_courier: boolean;
  customer_telegram: string | null;
  courier_telegram: string | null;
  courier_usdt: { trc20: string | null; bep20: string | null; erc20: string | null } | null;
}
