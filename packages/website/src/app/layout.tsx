import './globals.css';
import type { Metadata } from 'next';
import { AnalyticsProvider } from '@/components/AnalyticsProvider';

export const metadata: Metadata = {
  title: 'Voice Flow - Voice Layer for Your Desktop',
  description:
    'Voice Flow is the voice layer for your desktop, turning spoken thoughts into context-aware work right where your cursor is.',
  icons: { icon: '/logo.svg' },
  openGraph: {
    title: 'Voice Flow - Voice Layer for Your Desktop',
    description:
      'Voice-driven writing, input, and cross-app work for your desktop.',
    siteName: 'Voice Flow',
    type: 'website',
  },
  twitter: {
    card: 'summary_large_image',
    title: 'Voice Flow - Voice Layer for Your Desktop',
    description:
      'Voice-driven writing, input, and cross-app work for your desktop.',
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>
        <AnalyticsProvider>
          {children}
        </AnalyticsProvider>
      </body>
    </html>
  );
}
