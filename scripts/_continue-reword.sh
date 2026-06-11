#!/bin/bash
set -e
cd /home/chuan/context-osv6
git commit --amend -F /tmp/task23-msg.txt
git rebase --continue
