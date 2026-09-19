"""Aggregate only successful, revision-matched required quality jobs."""

from __future__ import annotations

import html

try:
    from ci_scope import plan_digest, validate_plan
except ModuleNotFoundError:
    from scripts.ci_scope import plan_digest, validate_plan

JOB_GROUPS = {"contracts": "contracts", "tooling": "tooling", "dependencies": "deps", "rust": "rust"}


def evaluate(plan: dict, needs: dict) -> list[str]:
    validate_plan(plan)
    if not isinstance(needs, dict) or set(needs) != {"changes", *JOB_GROUPS}:
        raise ValueError("aggregate requires exactly changes and all four quality jobs")
    expected = plan_digest(plan)
    failures = []
    for job, value in needs.items():
        if not isinstance(value, dict):
            failures.append(f"{job}: malformed job result")
            continue
        required = job == "changes" or plan["groups"][JOB_GROUPS[job]]
        wanted = "success" if required else "skipped"
        if value.get("result") != wanted:
            failures.append(f"{job}: expected {wanted}, got {value.get('result')!r}")
        if required:
            outputs = value.get("outputs", {})
            if not isinstance(outputs, dict) or outputs.get("tested_sha") != plan["tested_sha"]:
                failures.append(f"{job}: missing or mismatched tested SHA")
            if not isinstance(outputs, dict) or outputs.get("plan_sha256") != expected:
                failures.append(f"{job}: missing or mismatched plan digest")
    return failures


def summary(plan: dict, needs: dict, failures: list[str], run_url: str) -> str:
    def safe(value: object) -> str:
        return html.escape(str(value)).replace("|", "&#124;").replace("\n", " ")
    lines = ["## Quality verification", "", f"Run: {safe(run_url)}", ""]
    for key in ("event_name", "mode", "base_sha", "head_sha", "tested_sha", "help_base_sha"):
        lines.append(f"- {key}: `{safe(plan[key])}`")
    lines.extend([f"- Paths: {plan['path_count']}", f"- Path digest: `{plan['paths_sha256']}`",
                  f"- Plan digest: `{plan_digest(plan)}`", f"- Attempt: {plan['run_attempt']}",
                  f"- Reasons: {safe(', '.join(plan['reason_codes']))}", "",
                  "| Job | Required | Result |", "| --- | --- | --- |"])
    for job, group in JOB_GROUPS.items():
        value = needs.get(job, {})
        result = value.get("result", "missing") if isinstance(value, dict) else "invalid"
        lines.append(f"| {job} | {str(plan['groups'][group]).lower()} | {safe(result)} |")
    lines.extend(["", "Result: **failed**" if failures else "Result: **passed for the selected scope**"])
    lines.extend(f"- {safe(failure)}" for failure in failures)
    lines.extend(["", "Help review, native acceptance and the original workspace storage check remain separate obligations.", ""])
    return "\n".join(lines)
