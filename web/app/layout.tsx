import type { Metadata } from "next";
import Header from "@/components/Header";
import "./globals.css";

export const metadata: Metadata = {
  title: "TopUp Kilat — Top Up Game Otomatis",
  description:
    "Top up diamond, voucher, dan saldo game dengan proses otomatis. Bayar, unggah bukti, pesanan langsung diproses.",
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="id">
      <body className="min-h-screen text-slate-100 antialiased">
        <Header />
        <main className="mx-auto w-full max-w-6xl px-4 pb-24 pt-8">
          {children}
        </main>
        <footer className="border-t border-white/5 py-8">
          <div className="mx-auto w-full max-w-6xl px-4 text-center text-xs leading-relaxed text-slate-500">
            TopUp Kilat — pesanan diproses otomatis setelah pembayaran
            diverifikasi. Simpan nomor invoice untuk melacak pesanan.
          </div>
        </footer>
      </body>
    </html>
  );
}
