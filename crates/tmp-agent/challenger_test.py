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

def main():
    print("=== STARTING CHALLENGER VERIFICATION ===", flush=True)
    
    env = os.environ.copy()
    env["TMP_AGENT_PORT"] = "0"
    # Disable localharness execution to avoid CPU/process limits
    env["ANTIGRAVITY_HARNESS_PATH"] = "/nonexistent/harness"
    env.pop("GEMINI_API_KEY", None)
    
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
        print(f"[Server] {line.strip()}", flush=True)
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
        
    print(f"Detected tmp-agent port: {port}", flush=True)
    base_url = f"http://127.0.0.1:{port}"
    
    time.sleep(0.5)
    
    failed_checks = []

    # ----------------------------------------------------
    # 1. Thread Starvation & Concurrency Check
    # ----------------------------------------------------
    print("\n--- [CHECK 1] Concurrency & Thread Starvation ---", flush=True)
    status_latencies = []
    stop_polling = threading.Event()
    
    def status_poller():
        while not stop_polling.is_set():
            t_start = time.time()
            s, _ = send_get(f"{base_url}/status")
            t_elapsed = time.time() - t_start
            status_latencies.append(t_elapsed)
            time.sleep(0.01)
            
    poller_thread = threading.Thread(target=status_poller)
    poller_thread.start()
    
    time.sleep(0.2)  # Gather baseline
    baseline_max = max(status_latencies) if status_latencies else 0.0
    print(f"Baseline max status latency: {baseline_max:.4f}s", flush=True)
    
    # Spawn 10 concurrent requests to `/execute` running `sleep 2`
    def execute_sleeper():
        send_post(f"{base_url}/execute", {"command": "sleep", "args": ["2"]})
        
    threads = []
    num_requests = 10
    print(f"Sending {num_requests} concurrent /execute sleep requests...", flush=True)
    start_time = time.time()
    for _ in range(num_requests):
        t = threading.Thread(target=execute_sleeper)
        threads.append(t)
        t.start()
        
    for t in threads:
        t.join()
        
    elapsed = time.time() - start_time
    stop_polling.set()
    poller_thread.join()
    
    post_load_max = max(status_latencies) if status_latencies else 0.0
    print(f"Elapsed time for concurrent execution: {elapsed:.2f}s", flush=True)
    print(f"Max status latency observed during concurrent execution: {post_load_max:.4f}s", flush=True)
    
    if post_load_max > 0.1:
        print("FAIL: Thread starvation / latency spike observed during concurrency!", flush=True)
        failed_checks.append("Thread Starvation / Concurrency")
    else:
        print("PASS: No thread starvation. Server handled concurrent processes asynchronously.", flush=True)

    # ----------------------------------------------------
    # 2. Path Traversal Attacks Check
    # ----------------------------------------------------
    print("\n--- [CHECK 2] Path Traversal Attacks ---", flush=True)
    
    traversal_paths = [
        "/etc/hosts",
        "/etc/passwd",
        "../../etc/hosts",
        "../etc/hosts",
        "/Volumes/goldcoders/tmp/../../etc/hosts",
        "/var/tmp/../../etc/hosts",
        # Some path normalization bypasses
        "/Volumes/goldcoders/tmp/./../../etc/hosts",
        "/Volumes/goldcoders/tmp/../tmp/../../etc/hosts"
    ]
    
    read_blocked = True
    for path in traversal_paths:
        status, body = send_post(f"{base_url}/read_file", {"path": path})
        print(f"Read '{path}' -> Status {status}, Body: {body}", flush=True)
        if status == 200 and body.get("success"):
            print(f"VULNERABLE: Allowed reading of {path}!", flush=True)
            read_blocked = False
            break
        elif "Path traversal detected" not in body.get("error", ""):
            print(f"WARNING: Got error but not expected path traversal error message: {body.get('error')}", flush=True)
            
    write_blocked = True
    for path in traversal_paths:
        status, body = send_post(f"{base_url}/write_file", {"path": path, "content": "malicious content"})
        print(f"Write '{path}' -> Status {status}, Body: {body}", flush=True)
        if status == 200 and body.get("success"):
            print(f"VULNERABLE: Allowed writing to {path}!", flush=True)
            write_blocked = False
            break
            
    if read_blocked and write_blocked:
        print("PASS: Path traversal attacks blocked successfully.", flush=True)
    else:
        print("FAIL: Path traversal attacks were NOT blocked!", flush=True)
        failed_checks.append("Path Traversal Blocking")

    # ----------------------------------------------------
    # 3. SQL Queries Check (CTE, comments, mutating queries)
    # ----------------------------------------------------
    print("\n--- [CHECK 3] SQL Queries Check ---", flush=True)
    
    # Setup temporary SQLite database
    with tempfile.NamedTemporaryFile(suffix=".db", dir="/Volumes/goldcoders/tmp", delete=False) as tmp_db:
        db_path = tmp_db.name
        
    try:
        import sqlite3
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE test_users (id INTEGER PRIMARY KEY, username TEXT, score REAL)")
        cursor.execute("INSERT INTO test_users (username, score) VALUES ('alice', 95.5)")
        cursor.execute("INSERT INTO test_users (username, score) VALUES ('bob', 88.0)")
        conn.commit()
        conn.close()
        
        db_conn = {"sqlite_path": db_path, "pg_url": None}
        
        # A. WITH CTE query
        cte_query = "WITH cte AS (SELECT * FROM test_users WHERE score > 90) SELECT * FROM cte"
        status, body = send_post(f"{base_url}/db/query", {"query": cte_query, "connection": db_conn})
        print(f"CTE query -> Status {status}, Body: {body}", flush=True)
        cte_pass = (status == 200 and body.get("success") and len(body.get("rows", [])) == 1 and body["rows"][0]["username"] == "alice")
        
        # B. Queries with comments
        comment_queries = [
            "-- comment\nSELECT username FROM test_users",
            "SELECT username FROM test_users -- comment",
            "/* comment */ SELECT username FROM test_users",
            "SELECT username /* comment */ FROM test_users",
            "SELECT username FROM test_users; -- comment"
        ]
        comments_pass = True
        for q in comment_queries:
            status, body = send_post(f"{base_url}/db/query", {"query": q, "connection": db_conn})
            print(f"Comment query '{q}' -> Status {status}, Body: {body}", flush=True)
            if status != 200 or not body.get("success") or len(body.get("rows", [])) != 2:
                comments_pass = False
                print(f"FAIL: Comment query failed: {q}", flush=True)
                
        # C. Mutating queries (Blocked)
        mutating_queries = [
            "INSERT INTO test_users (username, score) VALUES ('charlie', 70.0)",
            "DELETE FROM test_users WHERE username='bob'",
            "UPDATE test_users SET score=100.0 WHERE username='bob'",
            "DROP TABLE test_users",
            "ALTER TABLE test_users ADD COLUMN is_admin INTEGER",
            "CREATE TABLE dummy (id INTEGER)",
            "TRUNCATE TABLE test_users",
            "REPLACE INTO test_users (id, username, score) VALUES (1, 'alice', 99.0)",
            # Case insensitive / formatting tests
            "insert into test_users (username) values ('hacker')",
            "SELECT * FROM test_users; DROP TABLE test_users",
            "WITH cte AS (SELECT 1) SELECT * FROM cte; INSERT INTO test_users (username) VALUES ('hacker')"
        ]
        mutating_blocked = True
        for q in mutating_queries:
            status, body = send_post(f"{base_url}/db/query", {"query": q, "connection": db_conn})
            print(f"Mutating query '{q}' -> Status {status}, Body: {body}", flush=True)
            if status == 200 and body.get("success"):
                print(f"VULNERABLE: Allowed mutating query: {q}", flush=True)
                mutating_blocked = False
                
        # D. Mutating keywords inside strings/comments (Allowed)
        allowed_queries = [
            "SELECT 'INSERT' AS action",
            "SELECT * FROM test_users WHERE username = 'DELETE'",
            "SELECT username FROM test_users -- INSERT comment"
        ]
        allowed_pass = True
        for q in allowed_queries:
            status, body = send_post(f"{base_url}/db/query", {"query": q, "connection": db_conn})
            print(f"Allowed query '{q}' -> Status {status}, Body: {body}", flush=True)
            if status != 200 or not body.get("success"):
                print(f"FAIL: Falsely blocked allowed query: {q}", flush=True)
                allowed_pass = False
                
        if cte_pass and comments_pass and mutating_blocked and allowed_pass:
            print("PASS: SQL validation (CTE, comments, and mutation block) is robust.", flush=True)
        else:
            print("FAIL: SQL validation failed one or more checks!", flush=True)
            failed_checks.append("SQL Validation")
            
    finally:
        if os.path.exists(db_path):
            os.remove(db_path)

    # ----------------------------------------------------
    # 4. Nonexistent SQLite Database Paths Check
    # ----------------------------------------------------
    print("\n--- [CHECK 4] Nonexistent SQLite Database Paths ---", flush=True)
    nonexistent_path = "/Volumes/goldcoders/tmp/nonexistent_test_db_999.db"
    if os.path.exists(nonexistent_path):
        os.remove(nonexistent_path)
        
    db_conn_nonexistent = {"sqlite_path": nonexistent_path, "pg_url": None}
    
    # Try /db/tables
    status_tables, body_tables = send_post(f"{base_url}/db/tables", {"connection": db_conn_nonexistent})
    print(f"/db/tables -> Status {status_tables}, Body: {body_tables}", flush=True)
    
    # Try /db/columns
    status_cols, body_cols = send_post(f"{base_url}/db/columns", {"table_name": "test", "connection": db_conn_nonexistent})
    print(f"/db/columns -> Status {status_cols}, Body: {body_cols}", flush=True)
    
    # Try /db/query
    status_query, body_query = send_post(f"{base_url}/db/query", {"query": "SELECT 1", "connection": db_conn_nonexistent})
    print(f"/db/query -> Status {status_query}, Body: {body_query}", flush=True)
    
    # Check if the file was created
    file_created = os.path.exists(nonexistent_path)
    print(f"Was nonexistent DB file created? {file_created}", flush=True)
    
    checks_passed = True
    if file_created:
        print("FAIL: Nonexistent DB file was created!", flush=True)
        os.remove(nonexistent_path)
        checks_passed = False
        
    if status_tables == 200 and body_tables.get("success"):
        print("FAIL: /db/tables succeeded on nonexistent database!", flush=True)
        checks_passed = False
        
    if status_cols == 200 and body_cols.get("success"):
        print("FAIL: /db/columns succeeded on nonexistent database!", flush=True)
        checks_passed = False
        
    if status_query == 200 and body_query.get("success"):
        print("FAIL: /db/query succeeded on nonexistent database!", flush=True)
        checks_passed = False
        
    if checks_passed:
        print("PASS: Query endpoints return error for nonexistent SQLite database paths instead of creating files.", flush=True)
    else:
        failed_checks.append("Nonexistent SQLite Path Handling")

    # ----------------------------------------------------
    # 5. Subagent Status Map Bounding Check
    # ----------------------------------------------------
    print("\n--- [CHECK 5] Subagent Bounding ---", flush=True)
    
    subagent_ids = []
    print("Spawning 105 subagents to check bounding...", flush=True)
    for i in range(105):
        if i % 10 == 0:
            print(f"Spawning subagent {i}...", flush=True)
        status, body = send_post(f"{base_url}/subagent", {"prompt": f"test prompt {i}"})
        if status == 200 and "subagent_id" in body:
            subagent_ids.append(body["subagent_id"])
        else:
            print(f"Warning: failed to spawn subagent {i}: Status {status}, Body: {body}", flush=True)
            
    print(f"Spawned {len(subagent_ids)} subagents.", flush=True)
    
    # The first 5 subagents should have been evicted (oldest first)
    evicted_correctly = True
    for i in range(5):
        sub_id = subagent_ids[i]
        status, body = send_get(f"{base_url}/subagent/{sub_id}")
        print(f"Checking subagent {i} (ID: {sub_id}) -> Status {status}, Body: {body}", flush=True)
        if status != 404:
            print(f"FAIL: Expected subagent {i} to be evicted (404), but got status {status}", flush=True)
            evicted_correctly = False
            
    # The last 100 subagents should be retained
    retained_correctly = True
    for i in range(5, 105):
        sub_id = subagent_ids[i]
        status, body = send_get(f"{base_url}/subagent/{sub_id}")
        if status != 200:
            print(f"FAIL: Expected subagent {i} to be retained (200), but got status {status}", flush=True)
            retained_correctly = False
            break
            
    if evicted_correctly and retained_correctly:
        print("PASS: Subagent status map is properly bounded to 100.", flush=True)
    else:
        print("FAIL: Subagent status map bounding failed!", flush=True)
        failed_checks.append("Subagent Map Bounding")

    # ----------------------------------------------------
    # Cleanup and report
    # ----------------------------------------------------
    print("\nStopping server...", flush=True)
    proc.terminate()
    proc.wait()
    
    print("\n=== CHALLENGER SUMMARY ===", flush=True)
    if failed_checks:
        print(f"FAILED CHECKS: {failed_checks}", flush=True)
        sys.exit(1)
    else:
        print("ALL VERIFICATIONS PASSED SUCCESSFULLY!", flush=True)
        sys.exit(0)

if __name__ == "__main__":
    main()
