import threading
from http.server import HTTPServer, BaseHTTPRequestHandler
import binascii

FIRMWARE_PATH = "firmware.bin"

with open(FIRMWARE_PATH, "rb") as f:
    FW = f.read()

FLASH_SIZE = len(FW)
TARGET_CRC = binascii.crc32(FW) & 0xFFFFFFFF


class OtaHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        client_ip, client_port = self.client_address
        print(f"[OTA] Request from {client_ip}:{client_port} for {self.path}")

        # Send firmware
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(FLASH_SIZE))
        self.send_header("target_crc", str(TARGET_CRC))
        self.end_headers()
        self.wfile.write(FW)
        self.wfile.flush()

        print(f"[OTA] Finished sending firmware to {client_ip}:{client_port}, shutting down server...")

        # Trigger server shutdown in a separate thread to avoid deadlock
        threading.Thread(
            target=self.server.shutdown,  # type: ignore[attr-defined]
            daemon=True,
        ).start()

    def log_message(self, fmt, *args):
        # Disable default HTTP logging
        return


if __name__ == "__main__":
    server = HTTPServer(("0.0.0.0", 8000), OtaHandler)
    print(f"Serving firmware.bin ({FLASH_SIZE} bytes, crc={TARGET_CRC}) on :8000")
    try:
        server.serve_forever()
    finally:
        server.server_close()
        print("[OTA] Server closed")

