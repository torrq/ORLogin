@echo off
powershell -Command "Start-Process python -ArgumentList 'record_input.py' -Verb RunAs"