# Running the held-out bench on one AWS EC2 Mac

This is a written plan only. Nothing here has been run, and no AWS resource exists yet. One `mac2-m2.metal` Dedicated Host runs the same bench, with one golden image restored before each arm. Because nothing personal is on that host, the Screen-Recording-dependent paths of Peekaboo and Cua can run there too.

## Cost and limits

- A `mac2-m2.metal` Dedicated Host costs about $0.88/h on demand in us-east-1, so about **$21 a day**. Apple's licence sets a **24-hour minimum** per allocation: the host cannot be released before 24 h, even if the instance stops sooner.
- Add the EBS root volume (200 GB gp3, under $1 a day) and AMI snapshots (about $0.05/GB-month).
- The model runs use George's Claude subscription, logged in on the host. That draws on the same usage limit as the Mac.

## George does first

1. **Credits.** In Billing → Credits, check that the credit covers Amazon EC2 Dedicated Host usage (usage type `HostUsage:mac2-m2`). Some promotional credits exclude it. Not confirmed for this account.
2. **Quota.** In Service Quotas → Amazon EC2, request "Running Dedicated mac2-m2 Hosts" ≥ 1 in the chosen region. The default is often 0, and approval can take a day, so request it now.
3. Pick a region and availability zone that offer the type:
   `aws ec2 describe-instance-type-offerings --location-type availability-zone --filters Name=instance-type,Values=mac2-m2.metal --region us-east-1`

## 1. Host, instance, disk

```bash
R=us-east-1; AZ=us-east-1a
HOST=$(aws ec2 allocate-hosts --region $R --instance-type mac2-m2.metal --availability-zone $AZ --quantity 1 \
  --query 'HostIds[0]' --output text)
AMI=$(aws ec2 describe-images --region $R --owners amazon \
  --filters 'Name=name,Values=amzn-ec2-macos-26*' 'Name=architecture,Values=arm64_mac' \
  --query 'sort_by(Images,&CreationDate)[-1].ImageId' --output text)   # macOS 26, the same major as the Mac (26.6.1)
aws ec2 create-key-pair --region $R --key-name heldout --query KeyMaterial --output text > ~/.ssh/heldout.pem && chmod 600 ~/.ssh/heldout.pem
SG=$(aws ec2 create-security-group --region $R --group-name heldout-ssh --description 'SSH from one IP' --query GroupId --output text)
aws ec2 authorize-security-group-ingress --region $R --group-id $SG --protocol tcp --port 22 --cidr <your-ip>/32
I=$(aws ec2 run-instances --region $R --image-id $AMI --instance-type mac2-m2.metal --key-name heldout \
  --security-group-ids $SG --placement HostId=$HOST \
  --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=200,VolumeType=gp3}' \
  --query 'Instances[0].InstanceId' --output text)
aws ec2 wait instance-status-ok --region $R --instance-ids $I     # a Mac takes 10–20 min to boot
IP=$(aws ec2 describe-instances --region $R --instance-ids $I --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
ssh -i ~/.ssh/heldout.pem ec2-user@$IP
```

On the host, grow APFS into the 200 GB volume (the AWS-documented steps):

```bash
PDISK=$(diskutil list physical external | head -n1 | cut -d' ' -f1)
APFSCONT=$(diskutil list physical external | grep Apple_APFS | tr -s ' ' | cut -d' ' -f8)
yes | sudo diskutil repairDisk $PDISK && sudo diskutil apfs resizeContainer $APFSCONT 0
```

## 2. GUI session over screen sharing

The accessibility APIs need a logged-in Aqua session, and a process started over SSH does not get one. So the bench runs from Terminal.app inside the screen-sharing session (in `tmux`, so SSH can attach to it), and Terminal is the app that holds the grants.

```bash
sudo passwd ec2-user                                         # choose a password; George keeps it
sudo launchctl enable system/com.apple.screensharing
sudo launchctl load -w /System/Library/LaunchDaemons/com.apple.screensharing.plist
sudo sysadminctl -autologin set -userName ec2-user -password '<that password>'   # the GUI session comes back after each restore
sudo pmset -a sleep 0 displaysleep 10 disksleep 0            # hn8 reads displaysleep; any fixed value is fine
```

From George's Mac: `ssh -i ~/.ssh/heldout.pem -L 5900:localhost:5900 ec2-user@$IP`, then open `vnc://localhost:5900` in Screen Sharing and log in as ec2-user.

## 3. Tools (in Terminal.app on the host)

The versions match the Mac. The commands are the pinned ones from `arms/ARMS.md`.

```bash
sudo softwareupdate --list | grep -i 'Command Line Tools'    # then: sudo softwareupdate -i '<that label>'  (swiftc for macctl and ho_probe)
brew install node@22 python@3.11 uv jq tmux gh               # Homebrew ships on the Amazon macOS AMIs
ln -sf "$(brew --prefix python@3.11)/bin/python3.11" ~/.local/bin/python3.11
curl -fsSL https://claude.ai/install.sh | bash -s 2.1.321    # the Mac's Claude Code version; check `claude --version` after
claude                                                       # George logs in with the subscription (/login), in the GUI session
# Google Chrome: install the stable DMG and record its version. The Mac has 157.0.8103.39; note any mismatch in the report.
```

Copy the bench from the Mac, keeping the paths under the home folder the scripts expect. Run on the Mac:

```bash
H=ec2-user@$IP; K="-e ssh -i ~/.ssh/heldout.pem"
W=.t3/worktrees/agent-desktop
rsync -a $K --exclude runs --exclude out ~/$W/perf-speed-cost/bench/ $H:$W/perf-speed-cost/bench/
rsync -a $K ~/$W/feat-unified-browser/bench/browser/ $H:$W/feat-unified-browser/bench/browser/
rsync -a $K ~/$W/bench-upstream/target/release/agent-desktop $H:$W/bench-upstream/target/release/
```

On the host, rerun the install block of `arms/ARMS.md` (same URLs, same checksums), recreate `bh-venv` with `browser-harness==0.1.13`, then:

```bash
cp -R ~/.agent-desktop-bench-target/heldout-tools/cua-driver-0.46.1/cua-driver-rs-0.46.1-darwin-arm64/CuaDriver.app /Applications/
```

## 4. Grants over screen sharing (George clicks)

In System Settings → Privacy & Security, on the host:
- **Accessibility**: Terminal, `/Applications/CuaDriver.app`, and the Peekaboo CLI at its `heldout-tools` path if `peekaboo permissions status --no-remote` reports it missing.
- **Screen Recording**: the same three.

Then run `bash heldout/preflight.sh`. Every line must be `ok`, and both permission blocks must show Accessibility true.

**Screen Recording policy.** On the Mac every arm runs with Screen Recording off. On the host, set `HELDOUT_SCREEN_RECORDING=1` for every arm: the "screenshots fail" line then leaves the native prompt, and screenshot-based tools work as their makers intend. Report host rows separately (`--host aws`), because the conditions differ from the Mac's.

## 5. Golden image

With everything installed, granted and preflight-clean, and with Chrome, TextEdit and Preview closed:

```bash
GOLD=$(aws ec2 create-image --region $R --instance-id $I --name heldout-golden-20261111 --query ImageId --output text)
aws ec2 wait image-available --region $R --image-ids $GOLD    # can take 30–60 min for a Mac
```

The TCC database and the auto-login setting are on the root volume, so the image keeps the grants.

## 6. Restore per arm, then run that arm

On the host, arms run in blocks: restore, then all 5 rounds of one arm, with task order still rotated. A restore takes the instance through a reboot. This keeps every arm's start state identical, which matters more than alternating arms on a dedicated machine that nothing else uses.

```bash
for ARM in ad-14 ad-up peekaboo cua bh pw; do
  T=$(aws ec2 create-replace-root-volume-task --region $R --instance-id $I --image-id $GOLD --delete-replaced-root-volume \
    --query 'ReplaceRootVolumeTask.ReplaceRootVolumeTaskId' --output text)
  until [ "$(aws ec2 describe-replace-root-volume-tasks --region $R --replace-root-volume-task-ids $T \
    --query 'ReplaceRootVolumeTasks[0].TaskState' --output text)" = succeeded ]; do sleep 60; done
  # Wait for auto-login, then in the host's Terminal (tmux), for a native arm:
  #   HELDOUT_HOST=aws HELDOUT_SCREEN_RECORDING=1 NATIVE_ARMS=$ARM bash heldout/run_all.sh claude-opus-5-5 0 4 native
  # or for a web arm:
  #   HELDOUT_HOST=aws WEB_ARMS=$ARM bash heldout/run_all.sh claude-opus-5-5 0 4 web
  rsync -a -e "ssh -i ~/.ssh/heldout.pem" ec2-user@$IP:.t3/worktrees/agent-desktop/perf-speed-cost/bench/heldout/runs/ \
    ~/.t3/worktrees/agent-desktop/perf-speed-cost/bench/heldout/runs-aws/$ARM/   # before the next restore wipes them
done
```

ad-14 and ad-up run both suites, so restore once for their native block and once for their web block. The time per restore is not measured; budget 20 min, so about 8 restores ≈ 2.7 h on top of the run time in `run_plan.md`. Ten restores fit in the 24 h minimum with room to spare.

## 7. Teardown

```bash
aws ec2 terminate-instances --region $R --instance-ids $I
aws ec2 release-hosts --region $R --host-ids $HOST          # only after 24 h from allocation
aws ec2 deregister-image --region $R --image-id $GOLD        # then delete its snapshots
```
