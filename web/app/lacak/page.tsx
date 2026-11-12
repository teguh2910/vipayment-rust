"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

export default function TrackPage() {
  const router = useRouter();
  const [invoice, setInvoice] = useState("");
  const [error, setError] = useState<string | null>(null);

  function submit(event: React.FormEvent) {
    event.preventDefault();
    const value = invoice.trim().toUpperCase();

    if (!value) {
      setError("Masukkan nomor invoice terlebih dahulu.");
      return;
    }

    router.push(`/invoice/${encodeURIComponent(value)}`);
  }

  return (
    <div className="mx-auto max-w-lg">
      <div className="rounded-2xl border border-white/10 bg-white/[0.03] p-6">
        <h1 className="text-xl font-bold text-white">Lacak pesanan</h1>
        <p className="mt-1 text-sm text-slate-400">
          Masukkan nomor invoice yang kamu terima setelah checkout.
        </p>

        <form onSubmit={submit} className="mt-5 space-y-3">
          <input
            value={invoice}
            onChange={(event) => {
              setInvoice(event.target.value);
              setError(null);
            }}
            placeholder="INV-260101-XXXXXXXX"
            className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 font-mono text-sm uppercase text-slate-100 placeholder:normal-case placeholder:text-slate-500 focus:border-cyan-400/60 focus:outline-none"
          />

          {error && <p className="text-xs text-rose-300">{error}</p>}

          <button
            type="submit"
            className="w-full rounded-xl bg-gradient-to-r from-indigo-500 to-cyan-400 px-4 py-2.5 text-sm font-semibold text-white"
          >
            Lacak invoice
          </button>
        </form>
      </div>
    </div>
  );
}
