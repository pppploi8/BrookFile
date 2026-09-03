#!/usr/bin/env python3
import argparse
import ctypes
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DEV_DIR = ROOT / ".dev"
STATE_FILE = DEV_DIR / "state.json"
IS_WINDOWS = os.name == "nt"

COMPONENTS = {
    "backend": {"cwd": ROOT / "backend", "cmd": ["cargo", "run"], "port": 3000, "log": "backend.log"},
    "frontend": {"cwd": ROOT / "frontend", "cmd": ["npm", "run", "dev"], "port": 8080, "log": "frontend.log"},
}


def load_state():
    try:
        data = json.loads(STATE_FILE.read_text(encoding="utf-8"))
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        return {}


def save_state(state):
    DEV_DIR.mkdir(exist_ok=True)
    tmp = STATE_FILE.with_suffix(".tmp")
    tmp.write_text(json.dumps(state, indent=2, ensure_ascii=False), encoding="utf-8")
    os.replace(tmp, STATE_FILE)


def process_info(pid):
    if pid is None:
        return False, None
    if IS_WINDOWS:
        return _windows_process_info(pid)
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False, None
    except PermissionError:
        return True, None
    return True, _linux_start_ticks(pid)


def _windows_process_info(pid):
    from ctypes import wintypes

    k32 = ctypes.windll.kernel32
    k32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    k32.OpenProcess.restype = wintypes.HANDLE
    k32.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    k32.GetExitCodeProcess.restype = wintypes.BOOL
    k32.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    k32.GetProcessTimes.restype = wintypes.BOOL
    k32.CloseHandle.argtypes = [wintypes.HANDLE]
    k32.CloseHandle.restype = wintypes.BOOL
    handle = k32.OpenProcess(0x1000, False, pid)
    if not handle:
        return False, None
    try:
        exit_code = wintypes.DWORD()
        if not k32.GetExitCodeProcess(handle, ctypes.byref(exit_code)):
            return False, None
        alive = exit_code.value == 259
        times = [wintypes.FILETIME() for _ in range(4)]
        token = None
        if k32.GetProcessTimes(handle, *map(ctypes.byref, times)):
            token = (times[0].dwHighDateTime << 32) | times[0].dwLowDateTime
        return alive, token
    finally:
        k32.CloseHandle(handle)


def _linux_start_ticks(pid):
    try:
        data = Path(f"/proc/{pid}/stat").read_text()
        fields = data.rsplit(")", 1)[1].split()
        return int(fields[19])
    except (OSError, IndexError, ValueError):
        return None


def record_status(record):
    if not record or record.get("pid") is None:
        return "stopped", False
    alive, token = process_info(record["pid"])
    if not alive:
        return "stale", False
    expected = record.get("token")
    if expected is not None and token is not None and expected != token:
        return "foreign", True
    return "running", True


def port_open(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.3):
            return True
    except OSError:
        return False


def start_one(name, state):
    conf = COMPONENTS[name]
    record = state.get(name)
    kind, _ = record_status(record)
    if kind == "running":
        print(f"[{name}] 已在运行 (pid={record['pid']})，跳过")
        return True
    if kind == "stale":
        print(f"[{name}] 清理过期记录 (pid={record['pid']})")
    elif kind == "foreign":
        print(f"[{name}] 旧记录 pid={record['pid']} 已被其他进程占用，忽略")
    state.pop(name, None)
    if port_open(conf["port"]):
        print(f"[{name}] 警告: 端口 {conf['port']} 已被占用，服务可能绑定失败")
    DEV_DIR.mkdir(exist_ok=True)
    log_path = DEV_DIR / conf["log"]
    exe = shutil.which(conf["cmd"][0]) or conf["cmd"][0]
    cmd = [exe] + conf["cmd"][1:]
    log_file = open(log_path, "w", encoding="utf-8", errors="replace")
    kwargs = {
        "cwd": str(conf["cwd"]),
        "stdin": subprocess.DEVNULL,
        "stdout": log_file,
        "stderr": subprocess.STDOUT,
    }
    if IS_WINDOWS:
        kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP | subprocess.CREATE_NO_WINDOW
    else:
        kwargs["start_new_session"] = True
    try:
        proc = subprocess.Popen(cmd, **kwargs)
    except OSError as exc:
        print(f"[{name}] 启动失败: {exc}")
        return False
    finally:
        log_file.close()
    _, token = process_info(proc.pid)
    state[name] = {
        "pid": proc.pid,
        "token": token,
        "started_at": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "cmd": " ".join(conf["cmd"]),
        "log": str(log_path),
    }
    print(f"[{name}] 已启动 (pid={proc.pid})，日志: {log_path}")
    return True


def stop_one(name, state):
    record = state.get(name)
    kind, _ = record_status(record)
    if kind == "stopped":
        print(f"[{name}] 未在运行")
        return True
    pid = record["pid"]
    if kind == "stale":
        print(f"[{name}] 进程已不存在 (pid={pid})，清理记录")
        state.pop(name, None)
        return True
    if kind == "foreign":
        print(f"[{name}] pid={pid} 已是其他进程，跳过终止，仅清理记录")
        state.pop(name, None)
        return True
    ok = kill_tree(pid)
    state.pop(name, None)
    if ok:
        print(f"[{name}] 已停止 (pid={pid})")
    else:
        print(f"[{name}] 停止失败 (pid={pid})，请手动处理")
    return ok


def kill_tree(pid):
    if IS_WINDOWS:
        result = subprocess.run(["taskkill", "/F", "/T", "/PID", str(pid)], capture_output=True)
        return result.returncode == 0
    if not _kill_group(pid, signal.SIGTERM):
        return not _pid_alive(pid)
    deadline = time.time() + 5
    while time.time() < deadline:
        if not _pid_alive(pid):
            return True
        time.sleep(0.2)
    _kill_group(pid, signal.SIGKILL)
    deadline = time.time() + 2
    while time.time() < deadline:
        if not _pid_alive(pid):
            return True
        time.sleep(0.2)
    return False


def _kill_group(pid, sig):
    try:
        if IS_WINDOWS:
            return False
        os.killpg(pid, sig)
        return True
    except ProcessLookupError:
        return True
    except PermissionError:
        return False


def _pid_alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def run_for_all(state, action):
    results = [action(name, state) for name in COMPONENTS]
    return all(results)


def cmd_start(_):
    state = load_state()
    ok = run_for_all(state, start_one)
    save_state(state)
    return 0 if ok else 1


def cmd_stop(_):
    state = load_state()
    ok = run_for_all(state, stop_one)
    save_state(state)
    return 0 if ok else 1


def cmd_restart(_):
    state = load_state()
    ok = run_for_all(state, stop_one)
    save_state(state)
    ok = run_for_all(state, start_one) and ok
    save_state(state)
    return 0 if ok else 1


def cmd_status(_):
    state = load_state()
    for name, conf in COMPONENTS.items():
        record = state.get(name)
        kind, _ = record_status(record)
        if kind == "running":
            pid = record["pid"]
            if port_open(conf["port"]):
                port_info = f"端口 {conf['port']} 已监听"
            else:
                port_info = f"端口 {conf['port']} 未监听（可能编译中或启动失败，见日志）"
            print(f"[{name}] 运行中  pid={pid}  启动于 {record.get('started_at')}  {port_info}")
        elif kind == "stale":
            print(f"[{name}] 已停止（残留记录 pid={record['pid']}）")
            state.pop(name, None)
        elif kind == "foreign":
            print(f"[{name}] 已停止（记录的 pid={record['pid']} 已被其他进程占用）")
            state.pop(name, None)
        else:
            print(f"[{name}] 已停止")
    save_state(state)
    print(f"状态文件: {STATE_FILE}")
    return 0


def main():
    for stream in (sys.stdout, sys.stderr):
        if stream is not None and hasattr(stream, "reconfigure"):
            stream.reconfigure(errors="replace")
    parser = argparse.ArgumentParser(description="BrookFile 前后端调试进程管理（非阻塞）")
    parser.add_argument("command", choices=["start", "status", "restart", "stop"],
                        help="start=后台启动 status=查看状态 restart=重启 stop=停止")
    args = parser.parse_args()
    sys.exit({
        "start": cmd_start,
        "stop": cmd_stop,
        "restart": cmd_restart,
        "status": cmd_status,
    }[args.command](args))


if __name__ == "__main__":
    main()
