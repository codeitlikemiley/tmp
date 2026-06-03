import subprocess
import time
import re
import sys
import json
import urllib.request
import urllib.error
import threading
import tempfile
import os

def send_post(url, payload):
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode('utf-8'),
        headers={'Content-Type': 'application/json'},
        method='POST'
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as response:
            return response.status, json.loads(response.read().decode('utf-8'))
    except urllib.error.HTTPError as e:
        try:
            body = json.loads(e.read().decode('utf-8'))
        except Exception:
            body = e.reason
        return e.code, body
    except Exception as e:
        return 500, str(e)

def send_get(url):
    try:
        with urllib.request.urlopen(url, timeout=10) as response:
            return response.status, json.loads(response.read().decode('utf-8'))
    except urllib.error.HTTPError as e:
        try:
            body = json.loads(e.read().decode('utf-8'))
        except Exception:
            body = e.reason
        return e.code, body
    except Exception as e:
        return 500, str(e)

def main():
    print("Starting tmp-agent server for integration testing...")
    
    env = os.environ.copy()
    env["TMP_AGENT_PORT"] = "0"
    
    server_bin = "/Volumes/goldcoders/tmp/target/debug/tmp-agent"
    if not os.path.exists(server_bin):
        print(f"Error: Binary {server_bin} does not exist. Run cargo build first.")
        sys.exit(1)
        
    proc = subprocess.Popen(
        [server_bin],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=env,
        text=True,
        bufsize=1
    )
    
    port = None
    for _ in range(100):
        line = proc.stdout.readline()
        print(f"[Server] {line.strip()}")
        match = re.search(r"Starting Axum server on 127\.0\.0\.1:(\d+)", line)
        if match:
            port = int(match.group(1))
            break
        if proc.poll() is not None:
            print("Server process exited prematurely.")
            sys.exit(1)
            
    if not port:
        print("Failed to detect server bind port from stdout.")
        proc.terminate()
        sys.exit(1)
        
    print(f"Detected tmp-agent port: {port}")
    base_url = f"http://127.0.0.1:{port}"
    
    time.sleep(0.5)
    
    tests_failed = []
    
    # 1. Test /status
    print("\n--- Testing /status ---")
    status, body = send_get(f"{base_url}/status")
    print(f"Status: {status}, Body: {body}")
    if status == 200 and body.get("status") == "ok":
        print("PASS")
    else:
        print("FAIL")
        tests_failed.append("/status")
        
    # 2. Test /chat
    print("\n--- Testing /chat ---")
    status, body = send_post(f"{base_url}/chat", {"message": "hello"})
    print(f"Status: {status}, Body: {body}")
    if status == 200 and "reply" in body:
        print("PASS")
    else:
        print("FAIL")
        tests_failed.append("/chat")

    # 3. Test /execute
    print("\n--- Testing /execute (valid) ---")
    status, body = send_post(f"{base_url}/execute", {"command": "echo", "args": ["hello-world"]})
    print(f"Status: {status}, Body: {body}")
    if status == 200 and body.get("success") and "hello-world" in body.get("stdout", ""):
        print("PASS")
    else:
        print("FAIL")
        tests_failed.append("/execute valid")

    # 4. Test file read/write
    print("\n--- Testing file read/write ---")
    with tempfile.NamedTemporaryFile(delete=False) as tmp:
        tmp_path = tmp.name
    
    try:
        test_content = "Integration test content: hello axum server"
        status, body = send_post(f"{base_url}/write_file", {"path": tmp_path, "content": test_content})
        print(f"Write Status: {status}, Body: {body}")
        
        status, body = send_post(f"{base_url}/read_file", {"path": tmp_path})
        print(f"Read Status: {status}, Body: {body}")
        if status == 200 and body.get("success") and body.get("content") == test_content:
            print("Read/Write: PASS")
        else:
            print("Read/Write: FAIL")
            tests_failed.append("/read_file /write_file")
    finally:
        if os.path.exists(tmp_path):
            os.remove(tmp_path)

    # 5. Correct path traversal test using /etc/hosts
    print("\n--- Testing /read_file path traversal (arbitrary file read) ---")
    status, body = send_post(f"{base_url}/read_file", {"path": "/etc/hosts"})
    print(f"Traversal Status: {status}, Body: {body}")
    if status == 200 and body.get("success") and "localhost" in body.get("content", ""):
        print("PASS (Path traversal / arbitrary file read confirmed)")
    else:
        print("FAIL (Could not read arbitrary system files)")
        tests_failed.append("path traversal")

    # 6. Test DB endpoints
    print("\n--- Testing SQLite DB endpoints ---")
    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as tmp_db:
        db_path = tmp_db.name
    
    try:
        import sqlite3
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, stock INTEGER)")
        cursor.execute("INSERT INTO products (name, stock) VALUES ('widget', 42)")
        conn.commit()
        conn.close()
        
        connection_payload = {"sqlite_path": db_path, "pg_url": None}
        
        # Test /db/query with comments and WITH
        status, body = send_post(f"{base_url}/db/query", {"query": "WITH tmp_prod AS (SELECT * FROM products) SELECT * FROM tmp_prod", "connection": connection_payload})
        print(f"Query WITH Status: {status}, Body: {body}")
        
        status, body = send_post(f"{base_url}/db/query", {"query": "/* comment */ SELECT * FROM products", "connection": connection_payload})
        print(f"Query Comment Prefix Status: {status}, Body: {body}")
        
    finally:
        if os.path.exists(db_path):
            os.remove(db_path)

    # 7. Concurrency / Thread-Blocking stress test with status polling latency measurement
    print("\n--- Concurrency / Thread-Blocking and Latency Stress Test ---")
    
    # We will poll status in a loop to see if the latency increases under load
    status_latencies = []
    stop_polling = threading.Event()
    
    def status_poller():
        while not stop_polling.is_set():
            t_start = time.time()
            s, _ = send_get(f"{base_url}/status")
            t_elapsed = time.time() - t_start
            status_latencies.append(t_elapsed)
            time.sleep(0.05)
            
    poller_thread = threading.Thread(target=status_poller)
    poller_thread.start()
    
    time.sleep(0.2) # record baseline latencies
    baseline_max = max(status_latencies) if status_latencies else 0.0
    print(f"Baseline max status latency: {baseline_max:.4f}s")
    
    # Send 16 concurrent execution requests that run sleep 1
    # This is higher than standard CPU core counts, so it will exhaust worker threads
    def worker():
        send_post(f"{base_url}/execute", {"command": "sleep", "args": ["1"]})
        
    threads = []
    num_requests = 16
    print(f"Sending {num_requests} concurrent blocking /execute requests...")
    start_time = time.time()
    for _ in range(num_requests):
        t = threading.Thread(target=worker)
        threads.append(t)
        t.start()
        
    for t in threads:
        t.join()
        
    elapsed = time.time() - start_time
    stop_polling.set()
    poller_thread.join()
    
    post_load_max = max(status_latencies) if status_latencies else 0.0
    print(f"Elapsed time for {num_requests} execution requests: {elapsed:.2f}s")
    print(f"Max status latency observed during load: {post_load_max:.4f}s")
    
    if post_load_max > 0.1:
        print(f"RESULT: Concurrency impact confirmed! Status request latency rose to {post_load_max:.4f}s (baseline {baseline_max:.4f}s) due to blocking handler thread starvation.")
    else:
        print("RESULT: No significant latency spike observed.")

    # Terminate server
    print("\nStopping server...")
    proc.terminate()
    proc.wait()
    
    print("\n=== INTEGRATION TEST SUMMARY ===")
    if tests_failed:
        print(f"FAILED TESTS: {tests_failed}")
        sys.exit(1)
    else:
        print("ALL TESTS COMPLETED SUCCESS")
        sys.exit(0)

if __name__ == "__main__":
    main()
