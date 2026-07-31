import http.client
import http.server
import json
from socketserver import ThreadingMixIn

# Configuration
LISTEN_PORT = 4010
UPSTREAM_HOST = "api.anthropic.com"
UPSTREAM_PORT = 443


class ThreadingHTTPServer(ThreadingMixIn, http.server.HTTPServer):
    """Handle requests concurrently so parallel tool calls don't block."""

    daemon_threads = True


class ProxyRequestHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"  # Required to support chunked streaming

    def log_message(self, format, *args):
        # Suppress the default http.server logging to keep our console clean
        pass

    def do_OPTIONS(self):
        # Pre-flight requests
        self.send_response(200)
        self.end_headers()

    def handle_request(self):
        # 1. Read incoming request body
        content_length = int(self.headers.get("Content-Length", 0))
        req_body = self.rfile.read(content_length) if content_length > 0 else b""

        # 2. Log the outgoing payload
        print(f"\n{'=' * 60}\n🚀 OUTGOING REQUEST ➔ {self.command} {self.path}")
        if req_body:
            try:
                parsed_json = json.loads(req_body.decode("utf-8"))
                print(json.dumps(parsed_json, indent=2))
            except Exception:
                print(req_body.decode("utf-8", errors="ignore"))

        # 3. Prepare headers for upstream
        upstream_headers = {}
        for key, value in self.headers.items():
            # Strip Accept-Encoding so the upstream doesn't compress the response, making it easy to read
            if key.lower() not in ["host", "accept-encoding"]:
                upstream_headers[key] = value

        upstream_headers["Host"] = UPSTREAM_HOST

        # 4. Forward the request upstream
        conn = http.client.HTTPSConnection(UPSTREAM_HOST, UPSTREAM_PORT)
        try:
            conn.request(
                self.command, self.path, body=req_body, headers=upstream_headers
            )
            upstream_res = conn.getresponse()

            # 5. Send response headers back to OpenCode
            self.send_response(upstream_res.status)
            is_chunked = False

            for key, value in upstream_res.getheaders():
                self.send_header(key, value)
                if key.lower() == "transfer-encoding" and value.lower() == "chunked":
                    is_chunked = True
            self.end_headers()

            # 6. Stream the response back and log it
            print(f"\n{'-' * 60}\n⬇ INCOMING RESPONSE ⬅ Status: {upstream_res.status}")

            response_chunks = []
            while True:
                # Read in small blocks to ensure smooth streaming
                chunk = upstream_res.read(4096)
                if not chunk:
                    break

                # http.client automatically decodes chunked responses.
                # If the original response was chunked, we must re-encode it for our client.
                if is_chunked:
                    chunk_size = hex(len(chunk))[2:].encode("utf-8")
                    self.wfile.write(chunk_size + b"\r\n" + chunk + b"\r\n")
                else:
                    self.wfile.write(chunk)

                self.wfile.flush()
                response_chunks.append(chunk)

            # Send the final empty chunk if chunked encoding was used
            if is_chunked:
                self.wfile.write(b"0\r\n\r\n")
                self.wfile.flush()

            # 7. Print the accumulated response
            full_response = b"".join(response_chunks).decode("utf-8", errors="ignore")
            if len(full_response) > 2000:
                print(full_response[:2000] + "\n\n... [TRUNCATED]")
            else:
                print(full_response)
            print("=" * 60 + "\n")

        except Exception as e:
            print(f"Error forwarding request: {e}")
            if not self.wfile.closed:
                self.send_error(500, "Proxy Error")
        finally:
            conn.close()

    # Route all HTTP methods to the same handler
    def do_GET(self):
        self.handle_request()

    def do_POST(self):
        self.handle_request()

    def do_PUT(self):
        self.handle_request()

    def do_DELETE(self):
        self.handle_request()


if __name__ == "__main__":
    server_address = ("127.0.0.1", LISTEN_PORT)
    httpd = ThreadingHTTPServer(server_address, ProxyRequestHandler)
    print(f"Stdlib Proxy running on http://127.0.0.1:{LISTEN_PORT}")
    print(f"Forwarding all traffic to https://{UPSTREAM_HOST}")
    print("Press Ctrl+C to stop.")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
        httpd.server_close()
