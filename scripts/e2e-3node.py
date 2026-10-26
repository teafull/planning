#!/usr/bin/env python3
"""
3 节点全流程自动化测试
用法: python3 scripts/e2e-3node.py

前提: cargo build 已完成
流程: 启动 3 实例(--bootstrap 组集群) → 各节点创建事件 → 验证同步 → 清理
"""
import subprocess, time, requests, json, os, sys, shutil, atexit

TEST_DIR = "/tmp/raft-e2e"
TOKEN = "e2eauto123"
BASE_PORT = 7878
processes = []

def find_binary():
    for c in ["src-tauri/target/debug/planning",
              os.path.expanduser("~/Desktop/zhangfeng/planning/src-tauri/target/debug/planning")]:
        if os.path.exists(c):
            return os.path.abspath(c)
    print("❌ cargo build 先"); sys.exit(1)

def cleanup():
    for p in processes:
        try: p.terminate(); p.wait(timeout=3)
        except: p.kill()
    shutil.rmtree(TEST_DIR, ignore_errors=True)

def start_leader():
    """节点1: --bootstrap 创建集群"""
    d = f"{TEST_DIR}/n1"; port = BASE_PORT
    os.makedirs(d, exist_ok=True)
    print(f"  节点1: port={port} bootstrap")
    p = subprocess.Popen(
        [BINARY, "--data-dir", d, "--port", str(port),
         "--bootstrap", "--token", TOKEN],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        start_new_session=True)
    processes.append(p)
    # 等 HTTP 就绪
    for _ in range(30):
        time.sleep(1)
        try:
            r = requests.get(f"http://127.0.0.1:{port}/cluster/status",
                           headers={"x-cluster-token": TOKEN}, timeout=2)
            if r.status_code == 200:
                print(f"  ✓ Leader 就绪 (port={port})")
                return port
        except: pass
    print("  ❌ Leader 启动超时"); sys.exit(1)

def start_follower(n):
    """节点 n: 先起应用, 再通过 HTTP join"""
    d = f"{TEST_DIR}/n{n}"; port = BASE_PORT + n - 1
    os.makedirs(d, exist_ok=True)
    print(f"  节点{n}: port={port} joining...")
    p = subprocess.Popen(
        [BINARY, "--data-dir", d, "--port", str(port)],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        start_new_session=True)
    processes.append(p)
    time.sleep(3)  # 等应用启动

    # 通过 Leader 的 /cluster/join 接口加入
    r = requests.post(f"http://127.0.0.1:{BASE_PORT}/cluster/join",
                      json={"token": TOKEN, "raftId": n,
                            "addr": f"127.0.0.1:{port}"}, timeout=5)
    if r.status_code != 200 or not r.json().get("ok"):
        print(f"  ❌ 节点{n} join 失败: {r.text}"); sys.exit(1)
    print(f"  ✓ 节点{n} 已加入")

    # 提升为投票成员
    time.sleep(2)
    r = requests.post(f"http://127.0.0.1:{BASE_PORT}/cluster/promote",
                      json={"raftId": n, "token": TOKEN}, timeout=5)
    if r.status_code == 200:
        print(f"  ✓ 节点{n} 已提升")
    time.sleep(2)
    return port

def create_event(port, title, date="2026-12-25"):
    """在指定节点创建事件"""
    r = requests.post(f"http://127.0.0.1:{port}/test/create-event",
                      json={"token": TOKEN, "date": date,
                            "startTime": 9.0, "endTime": 10.0,
                            "title": title, "eventType": "meeting"},
                      timeout=5)
    return r.json().get("ok", False)

def get_events(port):
    """获取指定节点的全部事件"""
    r = requests.get(f"http://127.0.0.1:{port}/test/events",
                     params={"token": TOKEN}, timeout=5)
    data = r.json()
    return data.get("events", [])

def wait_sync(port, expected_count, timeout=15):
    """等待指定节点事件数达到预期"""
    start = time.time()
    while time.time() - start < timeout:
        events = get_events(port)
        if len(events) >= expected_count:
            return True
        time.sleep(1)
    return False

def main():
    global BINARY
    BINARY = find_binary()
    atexit.register(cleanup)

    print("=" * 55)
    print("  3 节点全流程自动化测试")
    print("=" * 55)

    if os.path.exists(TEST_DIR):
        shutil.rmtree(TEST_DIR, ignore_errors=True)

    # 1. 组集群
    print("\n[1/4] 启动集群")
    leader_port = start_leader()
    p2 = start_follower(2)
    p3 = start_follower(3)
    time.sleep(3)

    # 2. 各节点创建事件
    print("\n[2/4] 创建事件(各节点独立创建)")
    tests = [
        (leader_port, "leader-created-meeting"),
        (p2, "follower2-created-task"),
        (p3, "follower3-created-deadline"),
    ]
    for port, title in tests:
        ok = create_event(port, title)
        status = "✓" if ok else "❌"
        print(f"  {status} 节点:{port} 创建 '{title}'")
        if not ok:
            print("  ❌ 创建失败"); sys.exit(1)
    time.sleep(3)  # 等复制

    # 3. 验证同步
    print("\n[3/4] 验证 3 节点同步")
    expected = 3
    all_pass = True
    for i, port in enumerate([leader_port, p2, p3], 1):
        ok = wait_sync(port, expected)
        events = get_events(port)
        titles = [e.get("title", "") for e in events]
        count = len(events)
        status = "✓" if ok else "❌"
        print(f"  {status} 节点{i}(port={port}): {count} 条事件")
        if ok:
            for t in sorted(titles):
                print(f"       - {t}")
        else:
            all_pass = False
            print(f"       期望 {expected} 条, 实际 {count} 条")
            print(f"       已有: {titles}")

    # 4. 结果
    print("\n[4/4] 结果")
    if all_pass:
        print("  ✅ 全部通过: 3 节点数据一致")
    else:
        print("  ❌ 同步异常")
        sys.exit(1)

    print("\n" + "=" * 55)
    print("  测试完成")
    print("=" * 55)

if __name__ == "__main__":
    main()
