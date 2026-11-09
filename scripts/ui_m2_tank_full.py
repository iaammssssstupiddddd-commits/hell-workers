"""Separate full-capacity Tank campaign; no inferred River or release authority."""
from scripts import ui_m2_tank_inflow as inflow

CASE, SCOPE = inflow.FULL_CASE, inflow.FULL_SCOPE
CHECKPOINTS = inflow.FULL_CHECKPOINTS
prepare_view = inflow.prepare_view
check_view = inflow.check_view
session_setting = inflow.session_setting
check_observation = inflow.check_observation
exercise = inflow.exercise


def source_contract(repo, source, codec):
    return inflow.construction.source_contract(repo, source, codec, profile=(CASE,SCOPE))


def planned_contract(args, repo):
    return inflow.planned_contract(args, repo, profile=(CASE,SCOPE))


def verify(raw, entries, client, inputs, contract):
    return inflow.verify(raw, entries, client, inputs, contract, profile=(CASE,SCOPE))
