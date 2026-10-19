#!/usr/bin/env python3
"""Send a set of realistic sample emails to a running Mailroom.

Uses only the Python standard library:

    python3 scripts/send-samples.py              # localhost:1025
    python3 scripts/send-samples.py mail.local 2525
"""

import base64
import smtplib
import struct
import sys
import zlib
from email.message import EmailMessage
from email.utils import make_msgid

HOST = sys.argv[1] if len(sys.argv) > 1 else "localhost"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 1025


def png(width, height, rgb):
    """A solid-colour PNG, generated without any imaging library."""
    row = b"\x00" + bytes(rgb) * width
    raw = row * height

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


TINY_PDF = base64.b64decode(
    "JVBERi0xLjQKMSAwIG9iago8PCAvVHlwZSAvQ2F0YWxvZyAvUGFnZXMgMiAwIFIgPj4KZW5kb2JqCjIgMCBvYmoKPDwgL1R5cGUgL1BhZ2VzIC9LaWRzIFszIDAgUl0gL0NvdW50IDEgPj4KZW5kb2JqCjMgMCBvYmoKPDwgL1R5cGUgL1BhZ2UgL1BhcmVudCAyIDAgUiAvTWVkaWFCb3ggWzAgMCAyMDAgMjAwXSA+PgplbmRvYmoKdHJhaWxlcgo8PCAvUm9vdCAxIDAgUiA+PgolJUVPRgo="
)


def password_reset():
    msg = EmailMessage()
    msg["From"] = "Acme <no-reply@acme.dev>"
    msg["To"] = "Alice Johnson <alice@example.com>"
    msg["Subject"] = "Reset your Acme password"
    msg.set_content(
        "Hi Alice,\n\nSomeone asked to reset your password. Use the link below within 30 minutes:\n\n"
        "https://app.acme.dev/reset-password?token=7f3c9a1e&email=alice%40example.com\n\n"
        "If you didn't ask for this, you can ignore this email.\n\n— The Acme team\n"
    )
    msg.add_alternative(
        """<html><body style="font-family:Helvetica,Arial,sans-serif;background:#f5f5f7;padding:32px">
        <div style="max-width:480px;margin:auto;background:#fff;border-radius:12px;padding:32px">
          <h2 style="margin-top:0">Reset your password</h2>
          <p>Hi Alice, someone asked to reset your password. This link expires in 30 minutes.</p>
          <p><a href="https://app.acme.dev/reset-password?token=7f3c9a1e&amp;email=alice%40example.com"
                style="display:inline-block;background:#111;color:#fff;padding:12px 20px;border-radius:8px;text-decoration:none">Choose a new password</a></p>
          <p style="color:#888;font-size:13px">If you didn't ask for this, ignore this email.
          <a href="https://acme.dev/help">Help center</a></p>
        </div></body></html>""",
        subtype="html",
    )
    return msg, None


def login_code():
    msg = EmailMessage()
    msg["From"] = "Acme Security <security@acme.dev>"
    msg["To"] = "bob@example.com"
    msg["Subject"] = "Your Acme sign-in code"
    msg.set_content("Your one-time sign-in code is 739201.\n\nIt expires in 10 minutes. Never share it with anyone.\n")
    return msg, None


def invoice():
    msg = EmailMessage()
    msg["From"] = "Billing <billing@acme.dev>"
    msg["To"] = "Finance <finance@globex.example>"
    msg["Subject"] = "Invoice INV-2026-0042 for October"
    msg.set_content("Hello,\n\nPlease find attached invoice INV-2026-0042 for $1,240.00, due 2026-11-15.\n\nThanks,\nAcme Billing\n")
    msg.add_attachment(TINY_PDF, maintype="application", subtype="pdf", filename="INV-2026-0042.pdf")
    msg.add_attachment(
        b"item,quantity,price\nPro plan,1,990.00\nExtra seats,5,250.00\n",
        maintype="text",
        subtype="csv",
        filename="line-items.csv",
    )
    return msg, None


def newsletter():
    msg = EmailMessage()
    msg["From"] = "The Changelog <news@acme.dev>"
    msg["To"] = "subscribers@acme.dev"
    msg["Subject"] = "What's new in October: dark mode, webhooks & more"
    msg["List-Unsubscribe"] = "<https://acme.dev/unsubscribe?u=123>, <mailto:unsubscribe@acme.dev>"
    banner = make_msgid(domain="acme.dev")
    msg.set_content("What's new in October\n\nDark mode is here, webhooks got retries, and the API is faster.\nRead more: https://acme.dev/changelog\n")
    msg.add_alternative(
        f"""<html><body style="margin:0;font-family:Georgia,serif;background:#fffaf2">
        <table width="100%" cellpadding="0" cellspacing="0"><tr><td align="center" style="padding:24px">
        <table width="560" cellpadding="0" cellspacing="0" style="background:#fff;border:1px solid #eee">
          <tr><td><img src="cid:{banner[1:-1]}" width="560" height="120" alt="October banner" style="display:block"></td></tr>
          <tr><td style="padding:28px">
            <h1 style="font-size:26px;margin:0 0 12px">What's new in October</h1>
            <p style="font-size:16px;line-height:1.6">🌙 <b>Dark mode</b> — easier on the eyes at night.</p>
            <p style="font-size:16px;line-height:1.6">🔁 <b>Webhook retries</b> — failed deliveries retry with backoff.</p>
            <p style="font-size:16px;line-height:1.6">⚡ <b>Faster API</b> — p95 latency down 38%.</p>
            <p><a href="https://acme.dev/changelog">Read the full changelog</a> · <a href="https://acme.dev/docs/webhooks">Webhook docs</a></p>
            <img src="https://placehold.co/520x80/png?text=Remote+image" width="520" alt="Remote image">
            <p style="font-size:12px;color:#999">You're receiving this because you signed up at acme.dev.
            <a href="https://acme.dev/unsubscribe?u=123">Unsubscribe</a></p>
          </td></tr>
        </table></td></tr></table></body></html>""",
        subtype="html",
    )
    msg.get_payload()[1].add_related(png(560, 120, (91, 91, 214)), maintype="image", subtype="png", cid=banner)
    return msg, None


def team_invite():
    msg = EmailMessage()
    msg["From"] = "Priya Sharma <priya@globex.example>"
    msg["To"] = "Carlos Diaz <carlos@example.com>"
    msg["Cc"] = "Dana Lee <dana@globex.example>"
    msg["Reply-To"] = "priya.sharma@globex.example"
    msg["Subject"] = "Priya invited you to join Globex on Acme"
    msg.set_content(
        "Hi Carlos,\n\nPriya invited you to the Globex workspace.\n\nAccept the invite: https://app.acme.dev/invites/accept?code=GX-48213\n"
    )
    # auditor@ is only in the envelope: Mailroom shows it as a blind copy.
    return msg, ["carlos@example.com", "dana@globex.example", "auditor@globex.example"]


def cron_report():
    msg = EmailMessage()
    msg["From"] = "cron@build-01.internal"
    msg["To"] = "ops@example.com"
    msg["Subject"] = "[build-01] Nightly backup completed"
    msg.set_content(
        "Backup finished at 03:12:44 UTC\n\n"
        "  databases   12   OK\n  buckets      4   OK\n  size     18.4 GB\n  duration  6m 12s\n\n"
        "Next run: 2026-10-20 03:00 UTC\n"
    )
    return msg, None


def international():
    msg = EmailMessage()
    msg["From"] = "Магазин <shop@example.ru>"
    msg["To"] = "Анна <anna@example.ru>"
    msg["Subject"] = "Ваш заказ №1042 подтверждён ✅"
    msg.set_content("Здравствуйте, Анна!\n\nВаш заказ №1042 подтверждён и будет отправлен завтра.\n\nご注文ありがとうございます。\n")
    return msg, None


def calendar_invite():
    msg = EmailMessage()
    msg["From"] = "Calendar <calendar@acme.dev>"
    msg["To"] = "alice@example.com, bob@example.com"
    msg["Subject"] = "Invitation: Sprint planning @ Tue Oct 20, 10:00"
    msg.set_content("You have been invited to Sprint planning.\nJoin: https://meet.acme.dev/sprint-planning\n")
    ics = (
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Acme//EN\r\nBEGIN:VEVENT\r\nUID:sprint-42@acme.dev\r\n"
        "DTSTART:20261020T100000Z\r\nDTEND:20261020T110000Z\r\nSUMMARY:Sprint planning\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    )
    msg.add_attachment(ics.encode(), maintype="text", subtype="calendar", filename="invite.ics")
    return msg, None


def no_subject():
    msg = EmailMessage()
    msg["From"] = "forgetful@example.com"
    msg["To"] = "alice@example.com"
    msg.set_content("Oops, I forgot the subject line.\n")
    return msg, None


SAMPLES = [
    cron_report,
    international,
    invoice,
    newsletter,
    calendar_invite,
    team_invite,
    no_subject,
    login_code,
    password_reset,
]


def main():
    with smtplib.SMTP(HOST, PORT) as smtp:
        smtp.ehlo("samples.mailroom.test")
        for build in SAMPLES:
            msg, envelope = build()
            smtp.send_message(msg, to_addrs=envelope)
            print(f"sent: {msg['Subject'] or '(no subject)'}")


if __name__ == "__main__":
    main()
