"""Typed Orca failure diagnosis with a deliberately small safe-repair set."""

from __future__ import annotations

from dataclasses import dataclass, asdict


@dataclass(frozen=True)
class Diagnosis:
    code: str
    disposition: str
    action: str | None
    message: str
    preserved: str


SAFE_ACTIONS = {"republish_snapshot", "ack_settled_receipt", "reconnect_same_session",
                "finalize_exited_terminal", "restart_idle_controller"}


def diagnose(observation: dict) -> dict:
    """Classify facts in safety priority order; absence is not proof of success."""
    if observation.get("dirtySource") is True or observation.get("headChanged") is True:
        result = Diagnosis("source_changed", "maintenance", None,
                           "承認対象が変更されています。自動修復しません。",
                           "作業場、差分、会話、台帳を保持")
    elif observation.get("unknownProcess") is True:
        result = Diagnosis("unknown_process", "maintenance", None,
                           "所有不明のprocessを検出しました。自動停止しません。",
                           "processと作業場を保持")
    elif observation.get("storageReady") is False:
        result = Diagnosis("storage_unavailable", "user_decision", None,
                           "保存領域の開始条件を満たしていません。",
                           "既存成果とreview-active cacheを保持")
    elif observation.get("runtimeReady") is False:
        result = Diagnosis("runtime_unavailable", "wait", None,
                           "Orca runtimeの再接続待ちです。",
                           "受付、Task、Dispatch、作業場を保持")
    elif observation.get("externalOnline") is False:
        result = Diagnosis("external_offline", "wait", None,
                           "外部サービスの復旧待ちです。既知のローカル作業は継続できます。",
                           "未送信operationとローカル成果を保持")
    elif observation.get("snapshotFresh") is False:
        result = Diagnosis("stale_snapshot", "auto_repair", "republish_snapshot",
                           "案件表示を正本台帳から再生成します。",
                           "実行状態は変更しない")
    elif observation.get("controllerRunning") is False:
        if observation.get("mutationInFlight") is True:
            result = Diagnosis("controller_stopped_during_write", "maintenance", None,
                               "外部write中にcontrollerが停止しました。read-backが必要です。",
                               "同じoperation IDと全成果を保持")
        else:
            result = Diagnosis("controller_stopped", "auto_repair", "restart_idle_controller",
                               "未確定writeがないためcontrollerを再開できます。",
                               "案件台帳とsessionを再利用")
    elif observation.get("terminalPresent") is False:
        if observation.get("processExited") is True and observation.get("settledReceipt") is True:
            result = Diagnosis("terminal_exited_settled", "auto_repair", "finalize_exited_terminal",
                               "終了受領書から担当終了を確定します。",
                               "scrollback要約と成果を保持")
        else:
            result = Diagnosis("terminal_missing", "maintenance", None,
                               "担当terminalの終了・消失を確定できません。",
                               "作業場とrole bindingを保持")
    elif observation.get("sessionConnected") is False:
        if observation.get("sameSession") is True:
            result = Diagnosis("session_disconnected", "auto_repair", "reconnect_same_session",
                               "同じsessionへ再接続します。",
                               "Task、Dispatch、generationを再利用")
        else:
            result = Diagnosis("session_mismatch", "maintenance", None,
                               "別sessionへの置換は許可されません。",
                               "元sessionと作業場を保持")
    elif observation.get("receiptAcknowledged") is False and observation.get("settledReceipt") is True:
        result = Diagnosis("ack_pending", "auto_repair", "ack_settled_receipt",
                           "確定済み受領書のACKを再投影します。",
                           "外部writeと成果は変更しない")
    elif observation.get("bridgeAccepted") is False:
        result = Diagnosis("bridge_refused", "maintenance", None,
                           "Task bridgeが担当権限を拒否しました。新規配車せず確認します。",
                           "Run、Task、Dispatch、terminalを保持")
    else:
        result = Diagnosis("healthy", "none", None, "再照合済みです。", "現状態を維持")
    return asdict(result)


def repair(diagnosis: dict, handlers: dict[str, object]) -> object:
    """Run only an explicitly supplied safe handler for an exact diagnosis."""
    action = diagnosis.get("action")
    if diagnosis.get("disposition") != "auto_repair" or action not in SAFE_ACTIONS:
        raise ValueError("diagnosis does not authorize automatic repair")
    handler = handlers.get(action)
    if not callable(handler):
        raise ValueError("safe repair handler is unavailable")
    return handler()
