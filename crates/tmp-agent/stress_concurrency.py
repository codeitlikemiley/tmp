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
from pathlib import Path

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
        return 500, {"success": False, "error": str(e)}

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
        return 500, {"success": False, "error": str(e)}

def start_server(env_overrides=None):
    env = os.environ.copy()
    env["TMP_AGENT_PORT"] = "0"
    env["ANTIGRAVITY_HARNESS_PATH"] = "/nonexistent/harness"
    env.pop("GEMINI_API_KEY", None)
    if env_overrides:
        env.update(env_overrides)
        
    server_bin = "/Volumes/goldcoders/tmp/target/debug/tmp-agent"
    if not os.path.exists(server_bin):
        print(f"Error: Binary {server_bin} does not exist. Run cargo build first.", flush=True)
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
        # print(f"[Server] {line.strip()}", flush=True)
        match = re.search(r"Starting Axum server on 127\.0\.0\.1:(\d+)", line)
        if match:
            port = int(match.group(1))
            break
        if proc.poll() is not None:
            print("Server process exited prematurely.", flush=True)
            sys.exit(1)
            
    if not port:
        print("Failed to detect server bind port from stdout.", flush=True)
        proc.terminate()
        sys.exit(1)
        
    return proc, port

def test_invalid_port_fallback():
    print("\n--- Testing Invalid Port Fallback ---", flush=True)
    # 1. Invalid port 'abc' should fallback to 0 and bind successfully
    proc, port = start_server({"TMP_AGENT_PORT": "abc"})
    print(f"Fallback bound to port: {port}", flush=True)
    proc.terminate()
    proc.wait()
    
    # 2. Out of range port '999999' should fallback to 0 and bind successfully
    proc, port = start_server({"TMP_AGENT_PORT": "999999"})
    print(f"Fallback bound to port: {port}", flush=True)
    proc.terminate()
    proc.wait()
    print("PASS: Invalid port fallback checks passed.", flush=True)

def run_stress_and_edge_tests():
    print("\n--- Starting Server for Stress and Edge Tests ---", flush=True)
    proc, port = start_server()
    base_url = f"http://127.0.0.1:{port}"
    time.sleep(0.5)
    
    failed_checks = []
    
    # Create temp DB for testing query endpoints
    with tempfile.NamedTemporaryFile(suffix=".db", dir="/Volumes/goldcoders/tmp", delete=False) as tmp_db:
        db_path = tmp_db.name
        
    try:
        import sqlite3
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, balance REAL)")
        cursor.execute("INSERT INTO users (name, balance) VALUES ('Alice', 1000.0)")
        cursor.execute("INSERT INTO users (name, balance) VALUES ('Bob', 500.0)")
        conn.commit()
        conn.close()
        
        db_conn = {"sqlite_path": db_path, "pg_url": None}
        
        # ----------------------------------------------------
        # Stress Test: 100 concurrent requests to /log
        # ----------------------------------------------------
        print("\n--- [Stress Test 1] 100 Concurrent Requests to /log ---", flush=True)
        log_latencies = []
        threads = []
        
        def log_requester(idx):
            t_start = time.time()
            status, body = send_post(f"{base_url}/log", {"level": "info", "message": f"Stress log message {idx}"})
            t_elapsed = time.time() - t_start
            log_latencies.append(t_elapsed)
            if status != 200 or not body.get("success"):
                print(f"Failed log request {idx}: status {status}, body {body}", flush=True)
                failed_checks.append(f"Log stress request {idx}")
                
        for i in range(100):
            t = threading.Thread(target=log_requester, args=(i,))
            threads.append(t)
            t.start()
            
        for t in threads:
            t.join()
            
        print(f"Completed 100 log requests. Avg latency: {sum(log_latencies)/len(log_latencies):.4f}s, Max latency: {max(log_latencies):.4f}s", flush=True)
        
        # ----------------------------------------------------
        # Stress Test: 100 concurrent requests to /db/query
        # ----------------------------------------------------
        print("\n--- [Stress Test 2] 100 Concurrent Requests to /db/query ---", flush=True)
        query_latencies = []
        threads = []
        
        def query_requester(idx):
            t_start = time.time()
            status, body = send_post(f"{base_url}/db/query", {
                "query": "SELECT * FROM users",
                "connection": db_conn
            })
            t_elapsed = time.time() - t_start
            query_latencies.append(t_elapsed)
            if status != 200 or not body.get("success"):
                print(f"Failed query request {idx}: status {status}, body {body}", flush=True)
                failed_checks.append(f"Query stress request {idx}")
                
        for i in range(100):
            t = threading.Thread(target=query_requester, args=(i,))
            threads.append(t)
            t.start()
            
        for t in threads:
            t.join()
            
        print(f"Completed 100 query requests. Avg latency: {sum(query_latencies)/len(query_latencies):.4f}s, Max latency: {max(query_latencies):.4f}s", flush=True)
        
        # ----------------------------------------------------
        # Edge Cases: Empty strings and invalid schemas
        # ----------------------------------------------------
        print("\n--- [Edge Cases] Input Validation ---", flush=True)
        
        # A. Execute empty command
        status, body = send_post(f"{base_url}/execute", {"command": "", "args": []})
        print(f"Empty command -> Status {status}, Body: {body}", flush=True)
        if status != 200 or body.get("success"):
            failed_checks.append("Empty command allowed or crashed")
            
        # B. Read empty path
        status, body = send_post(f"{base_url}/read_file", {"path": ""})
        print(f"Empty read path -> Status {status}, Body: {body}", flush=True)
        if status != 200 or body.get("success"):
            failed_checks.append("Empty read path allowed or crashed")
            
        # C. Write empty path
        status, body = send_post(f"{base_url}/write_file", {"path": "", "content": "test"})
        print(f"Empty write path -> Status {status}, Body: {body}", flush=True)
        if status != 200 or body.get("success"):
            failed_checks.append("Empty write path allowed or crashed")
            
        # D. Subagent empty prompt
        status, body = send_post(f"{base_url}/subagent", {"prompt": ""})
        print(f"Empty prompt -> Status {status}, Body: {body}", flush=True)
        if status != 200:
            failed_checks.append("Empty subagent prompt failed")
            
        # E. DB tables missing connection payload
        status, body = send_post(f"{base_url}/db/tables", {"connection": {}})
        print(f"Missing DB connection keys -> Status {status}, Body: {body}", flush=True)
        if status != 400 or body.get("success"):
            failed_checks.append("Empty DB connection keys allowed")
            
        # F. Mutating query check with spaces, newlines, and comments
        mutating_query_comment = "SELECT 1; -- comment \n UPDATE users SET balance = 9999"
        status, body = send_post(f"{base_url}/db/query", {
            "query": mutating_query_comment,
            "connection": db_conn
        })
        print(f"Mutating query with comment -> Status {status}, Body: {body}", flush=True)
        if status == 200 and body.get("success"):
            failed_checks.append("Mutating query bypass via comments")
            
    finally:
        # Clean up database
        if os.path.exists(db_path):
            os.remove(db_path)
        # Terminate server
        proc.terminate()
        proc.wait()
        
    print("\n--- Summary of Stress and Edge Checks ---", flush=True)
    if failed_checks:
        print(f"Failed checks: {failed_checks}", flush=True)
        sys.exit(1)
    else:
        print("PASS: All stress and edge check scenarios completed successfully.", flush=True)
        sys.exit(0)

if __name__ == "__main__":
    test_invalid_port_fallback()
    run_stress_and_edge_tests()
