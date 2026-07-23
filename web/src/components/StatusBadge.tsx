import type { OrderStatus } from "@/lib/types";

const STYLES: Record<OrderStatus, string> = {
  OPEN: "bg-sky-950 text-sky-300",
  ACCEPTED: "bg-indigo-950 text-indigo-300",
  AWAITING_PAYMENT: "bg-amber-950 text-amber-300",
  PAID: "bg-emerald-950 text-emerald-300",
  COMPLETED: "bg-emerald-900 text-emerald-200",
  CANCELLED: "bg-zinc-800 text-zinc-400",
};

const LABELS: Record<OrderStatus, string> = {
  OPEN: "Open",
  ACCEPTED: "Accepted",
  AWAITING_PAYMENT: "Awaiting payment",
  PAID: "Paid",
  COMPLETED: "Completed",
  CANCELLED: "Cancelled",
};

export default function StatusBadge({ status }: { status: OrderStatus }) {
  return <span className={`rounded px-2 py-0.5 text-xs font-medium ${STYLES[status]}`}>{LABELS[status]}</span>;
}
