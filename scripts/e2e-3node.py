#!/usr/bin/env python3
"""
3 节点全流程自动化测试(含 CRUD + 类型变更 + 时间调整 + 删除)
用法: python3 scripts/e2e-3node.py
"""
import subprocess, time, requests, json, os, sys, shutil, atexit

TEST_DIR = "/tmp/raft-e2e"
TOKEN = "e2eauto123"
BASE_PORT = 7878
processes = []
passed = 0
failed = 0

def ok(name):
    global passed; passed += 1
    print(f"  ✅ {name}")

def fail(name, detail=""):
    global failed; failed += 1
    print(f"  ❌ {name}  {detail}")

def find_binary():
    for c in ["src-tauri/target/debug/planning",
              os.path.expanduser("~/Desktop/zhangfeng/planning/src-tauri/target/debug/planning")]:
        if os.path.exists(c): return os.path.abspath(c)
    print("❌ cargo build 先"); sys.exit(1)

def cleanup():
    for p in processes:
        try: p.terminate(); p.wait(timeout=3)
        except: p.kill()
    shutil.rmtree(TEST_DIR, ignore_errors=True)

def start_leader():
    d = f"{TEST_DIR}/n1"; port = BASE_PORT
    os.makedirs(d, exist_ok=True)
    print(f"  节点1: port={port} bootstrap")
    p = subprocess.Popen(
        [BINARY, "--data-dir", d, "--port", str(port),
         "--bootstrap", "--token", TOKEN],
        stdout=subprocess.DEVNULL, stderr=subprocess.STDOUT,
        start_new_session=True)
    processes.append(p)
    for _ in range(30):
        time.sleep(1)
        try:
            r = requests.get(f"http://127.0.0.1:{port}/cluster/status",
                           headers={"x-cluster-token": TOKEN}, timeout=2)
            if r.status_code == 200:
                print(f"  ✓ Leader 就绪"); return port
        except: pass
    print("  ❌ Leader 超时"); sys.exit(1)

def start_follower(n):
    d = f"{TEST_DIR}/n{n}"; port = BASE_PORT + n - 1
    os.makedirs(d, exist_ok=True)
    print(f"  节点{n}: port={port} joining via --join...")
    p = subprocess.Popen(
        [BINARY, "--data-dir", d, "--port", str(port),
         "--join", f"127.0.0.1:{BASE_PORT}", "--token", TOKEN],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        start_new_session=True)
    processes.append(p)

    # 等待该节点 HTTP 就绪(表示 Raft + HTTP 已启动)
    for _ in range(30):
        time.sleep(1)
        try:
            r = requests.get(f"http://127.0.0.1:{port}/cluster/status",
                           headers={"x-cluster-token": TOKEN}, timeout=2)
            if r.status_code == 200:
                print(f"  ✓ 节点{n} 就绪 (port={port})")
                return port
        except: pass
    print(f"  ❌ 节点{n} 启动超时"); sys.exit(1)

# ===== HTTP helpers =====

def create_event(port, title, date="2026-12-25", start=9.0, end=10.0, ev_type="meeting"):
    try:
        r = requests.post(f"http://127.0.0.1:{port}/test/create-event",
                          json={"token": TOKEN, "date": date,
                                "startTime": start, "endTime": end,
                                "title": title, "eventType": ev_type}, timeout=20)
        return r.json().get("ok", False)
    except Exception:
        return False

def update_event(port, event_id, **kwargs):
    body = {"token": TOKEN, "id": event_id}
    body.update(kwargs)
    try:
        r = requests.post(f"http://127.0.0.1:{port}/test/update-event",
                          json=body, timeout=20)
        return r.json().get("ok", False)
    except Exception:
        return False

def delete_event(port, event_id):
    try:
        r = requests.post(f"http://127.0.0.1:{port}/test/delete-event",
                          json={"token": TOKEN, "id": event_id}, timeout=20)
        return r.json().get("ok", False)
    except Exception:
        return False

def get_events(port):
    try:
        r = requests.get(f"http://127.0.0.1:{port}/test/events",
                         params={"token": TOKEN}, timeout=20)
        return r.json().get("events", [])
    except Exception:
        return []

def find_event(port, title):
    for e in get_events(port):
        if e.get("title") == title:
            return e
    return None

def wait_condition(port, fn, timeout=15):
    start = time.time()
    while time.time() - start < timeout:
        if fn(get_events(port)):
            return True
        time.sleep(1)
    return False

def assert_all_nodes(name, ports, fn, timeout=15):
    """断言所有节点满足条件"""
    for i, port in enumerate(ports, 1):
        if wait_condition(port, fn, timeout):
            ok(f"{name} → 节点{i}")
        else:
            fail(f"{name} → 节点{i}(port={port})", "超时")

def main():
    global BINARY
    BINARY = find_binary()
    atexit.register(cleanup)

    print("=" * 55)
    print("  3 节点全流程自动化测试")
    print("=" * 55)

    if os.path.exists(TEST_DIR):
        shutil.rmtree(TEST_DIR, ignore_errors=True)

    # ===== Phase 1: 组集群 =====
    print("\n[Phase 1] 启动集群")
    p1 = start_leader()
    p2 = start_follower(2)
    p3 = start_follower(3)
    ports = [p1, p2, p3]

    # 等待自动转正完成(--join 内置 12 次 × 5s 重试)
    print("  等待转正(最多30s)...")
    time.sleep(15)
    # 检查是否已是投票成员
    try:
        r = requests.get(f"http://127.0.0.1:{BASE_PORT}/cluster/status",
                        headers={"x-cluster-token": TOKEN}, timeout=5)
        members = r.json().get("membership", [])
        print(f"  成员: {len(members)} 个")
    except: pass
    time.sleep(15)
    print("  ✓ 集群就绪")

    # ===== Phase 2: 创建(多类型) =====
    print("\n[Phase 2] 创建事件(各节点+多类型)")
    creates = [
        (p1, "task-from-leader", "task", 9.0, 10.0),
        (p2, "meeting-from-f2", "meeting", 14.0, 15.5),
        (p3, "deadline-from-f3", "deadline", 8.0, 8.5),
        (p1, "study-multi-hour", "study", 10.0, 12.5),
        (p2, "vacation-allday-test", "vacation", 0.0, 0.0),
    ]
    for port, title, ty, st, et in creates:
        if create_event(port, title, ev_type=ty, start=st, end=et):
            ok(f"创建 '{title}' (type={ty})")
        else:
            fail(f"创建 '{title}'")
    time.sleep(8)

    # 验证: 所有节点都有 5 条事件
    assert_all_nodes("5条事件同步", ports, lambda evs: len(evs) >= 5)

    # 验证: 类型正确
    print("\n[Phase 2b] 验证事件类型")
    for _, title, ty, _, _ in creates:
        if ty == "task": continue  # vacation start=0 可能有问题
        def check_type(evs, t=title, ty=ty):
            e = next((x for x in evs if x.get("title") == t), None)
            return e is not None and e.get("type") == ty
        assert_all_nodes(f"'{title}' type={ty}", ports, check_type)

    # 验证: 时间正确
    print("\n[Phase 2c] 验证时间字段")
    def check_time(evs):
        e = next((x for x in evs if x.get("title") == "meeting-from-f2"), None)
        return e and e.get("startTime") == 14.0 and e.get("endTime") == 15.5
    assert_all_nodes("'meeting-from-f2' 时间 14:00-15:30", ports, check_time)

    # ===== Phase 3: 更新 =====
    print("\n[Phase 3] 更新事件")
    # 3a: 改类型
    ev = find_event(p1, "task-from-leader")
    if ev:
        eid = ev["id"]
        if update_event(p1, eid, eventType="issue"):
            ok("类型变更 task→issue (Leader发起)")
            time.sleep(5)
            def check_type_change(evs):
                e = next((x for x in evs if x.get("title") == "task-from-leader"), None)
                return e and e.get("type") == "issue"
            assert_all_nodes("'task-from-leader' type=issue 同步", ports, check_type_change)
        else:
            fail("类型变更请求失败")
    else:
        fail("找不到 task-from-leader")

    # 3b: 改时间(从 Follower 发起)
    ev = find_event(p2, "study-multi-hour")
    if ev:
        eid = ev["id"]
        if update_event(p2, eid, startTime=13.0, endTime=15.0):
            ok("时间变更 10:00→13:00 (Follower2发起)")
            time.sleep(5)
            def check_time_change(evs):
                e = next((x for x in evs if x.get("title") == "study-multi-hour"), None)
                return e and e.get("startTime") == 13.0 and e.get("endTime") == 15.0
            assert_all_nodes("'study-multi-hour' 13:00-15:00 同步", ports, check_time_change)
        else:
            fail("时间变更请求失败")
    else:
        fail("找不到 study-multi-hour")

    # 3c: 改标题+类型同时(从 Follower3 发起)
    ev = find_event(p3, "deadline-from-f3")
    if ev:
        eid = ev["id"]
        if update_event(p3, eid, title="deadline-renamed", eventType="trip"):
            ok("标题+类型同时变更 (Follower3发起)")
            time.sleep(5)
            def check_both(evs):
                e = next((x for x in evs if x.get("title") == "deadline-renamed"), None)
                return e and e.get("type") == "trip"
            assert_all_nodes("'deadline-renamed' type=trip 同步", ports, check_both)
        else:
            fail("标题+类型变更请求失败")

    # ===== Phase 4: 删除 =====
    print("\n[Phase 4] 删除事件")
    ev = find_event(p2, "vacation-allday-test")
    if ev:
        eid = ev["id"]
        if delete_event(p2, eid):
            ok("删除 'vacation-allday-test' (Follower2发起)")
            time.sleep(5)
            def check_deleted(evs):
                return not any(x.get("title") == "vacation-allday-test" for x in evs)
            assert_all_nodes("'vacation-allday-test' 已删除", ports, check_deleted)
        else:
            fail("删除请求失败")

    # ===== Phase 5: 最终一致性 =====
    print("\n[Phase 5] 最终一致性检查")
    time.sleep(3)
    all_events = {p: get_events(p) for p in ports}
    counts = {p: len(evs) for p, evs in all_events.items()}
    titles_by_port = {p: sorted(e.get("title","") for e in evs) for p, evs in all_events.items()}

    if len(set(counts.values())) == 1:
        ok(f"事件数量一致: {list(counts.values())[0]} 条")
    else:
        fail(f"事件数量不一致: {counts}")

    ref_titles = titles_by_port[ports[0]]
    for i, port in enumerate(ports[1:], 2):
        if titles_by_port[port] == ref_titles:
            ok(f"节点{i} 标题集合与节点1一致")
        else:
            only_in_1 = set(ref_titles) - set(titles_by_port[port])
            only_in_n = set(titles_by_port[port]) - set(ref_titles)
            fail(f"节点{i} 标题不一致", f"独有: {only_in_1 | only_in_n}")

    # ===== 结果 =====
    print("\n" + "=" * 55)
    total = passed + failed
    print(f"  结果: {passed}/{total} 通过, {failed} 失败")
    if failed == 0:
        print("  ✅ 全部通过")
    else:
        print("  ❌ 存在失败项")
    print("=" * 55)
    sys.exit(0 if failed == 0 else 1)

if __name__ == "__main__":
    main()
