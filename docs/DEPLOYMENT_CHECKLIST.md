# RustAssistant Deployment Checklist

**Last Updated:** 2026-02-01  
**Target:** Raspberry Pi (ARM64) via Tailscale VPN  
**CI/CD:** GitHub Actions with Docker multi-arch builds

---

## 📋 Pre-Deployment Checklist

### 1. Local Development Setup
- [ ] Project builds successfully: `cargo build --release`
- [ ] All tests pass: `cargo test`
- [ ] Clippy checks pass: `cargo clippy`
- [ ] Format is correct: `cargo fmt --check`
- [ ] Web UI works locally: `./run.sh server`
- [ ] Environment variables documented in `.env.example`

### 2. Raspberry Pi Server Preparation
- [ ] Raspberry Pi OS (64-bit) installed and updated
- [ ] SSH access configured
- [ ] Static IP or hostname configured (optional, Tailscale is primary)
- [ ] Minimum 8GB SD card (16GB+ recommended)
- [ ] Stable power supply (5V 3A recommended)
- [ ] Internet connectivity verified

### 3. Tailscale Network Setup
- [ ] Tailscale account created: https://login.tailscale.com/
- [ ] OAuth credentials generated for CI/CD:
  - Go to: https://login.tailscale.com/admin/settings/oauth
  - Click "Generate OAuth client"
  - Note: This is for GitHub Actions to connect to your tailnet
- [ ] Tailscale installed on Raspberry Pi (done by setup script)
- [ ] Tailscale connected: `sudo tailscale up`
- [ ] Tailscale IP noted: `tailscale ip -4`

### 4. Docker Hub Setup
- [ ] Docker Hub account created: https://hub.docker.com/
- [ ] Repository created: `YOUR_USERNAME/rustassistant`
- [ ] Access token generated:
  - Go to: https://hub.docker.com/settings/security
  - Click "New Access Token"
  - Name: "GitHub Actions"
  - Permissions: Read, Write, Delete
- [ ] Token saved securely

### 5. Discord Notifications (Optional)
- [ ] Discord server created or selected
- [ ] Webhook created:
  - Server Settings → Integrations → Webhooks
  - Click "New Webhook"
  - Name: "RustAssistant CI/CD"
  - Copy webhook URL
- [ ] Webhook URL saved securely

---

## 🚀 Deployment Steps

### Step 1: Prepare Raspberry Pi

SSH into your Raspberry Pi:
```bash
ssh pi@YOUR_PI_IP
# or via Tailscale
ssh pi@YOUR_TAILSCALE_IP
```

Download setup scripts:
```bash
# Clone repository or download scripts
git clone https://github.com/YOUR_USERNAME/rustassistant.git
cd rustassistant/scripts

# Make scripts executable
chmod +x setup-production-server.sh
chmod +x generate-secrets.sh
```

Run server setup:
```bash
sudo ./setup-production-server.sh
```

This will:
- ✅ Install Docker and Docker Compose
- ✅ Create `actions` user for CI/CD
- ✅ Configure SSH
- ✅ Setup UFW firewall
- ✅ Install Tailscale
- ✅ Optimize for Raspberry Pi
- ✅ Configure Docker log rotation

**IMPORTANT:** If on Raspberry Pi, you may need to reboot for cgroup changes:
```bash
sudo reboot
```

After reboot, connect Tailscale:
```bash
sudo tailscale up
```

### Step 2: Generate Secrets

Run secrets generation:
```bash
sudo ./generate-secrets.sh
```

This will:
- ✅ Generate SSH keys for `actions` user
- ✅ Detect Tailscale IP
- ✅ Generate application secrets
- ✅ Create credentials file with all secrets
- ✅ Provide GitHub Secrets format

**Save the credentials file location** - you'll need it for the next step!

View credentials:
```bash
cat /tmp/rustassistant_credentials_TIMESTAMP.txt
```

### Step 3: Configure GitHub Secrets

Go to your repository settings:
```
https://github.com/YOUR_USERNAME/rustassistant/settings/secrets/actions
```

Add these **REQUIRED** secrets:

| Secret Name | Value | Where to Find |
|-------------|-------|---------------|
| `TAILSCALE_OAUTH_CLIENT_ID` | OAuth client ID | Tailscale admin console → OAuth clients |
| `TAILSCALE_OAUTH_SECRET` | OAuth secret | Same location (copy immediately, shown once) |
| `PROD_TAILSCALE_IP` | 100.x.x.x | From credentials file or `tailscale ip -4` |
| `PROD_SSH_KEY` | SSH private key | From credentials file (entire key including BEGIN/END) |
| `PROD_SSH_USER` | `actions` | Default user created by setup script |
| `PROD_SSH_PORT` | `22` | Default SSH port (unless changed) |
| `DOCKER_USERNAME` | Your Docker Hub username | Your Docker Hub account |
| `DOCKER_TOKEN` | Docker Hub token | Generated in Docker Hub settings |

Add these **OPTIONAL** secrets:

| Secret Name | Value | Purpose |
|-------------|-------|---------|
| `DISCORD_WEBHOOK_ACTIONS` | Discord webhook URL | CI/CD notifications |
| `RUSTASSISTANT_API_KEY` | From credentials file | Application API key (if needed) |

**Security Note:** After copying secrets to GitHub, delete the credentials file:
```bash
sudo rm /tmp/rustassistant_credentials_*.txt
```

### Step 4: Update Docker Image Name

In `.github/workflows/ci-cd.yml`, update:
```yaml
env:
    IMAGE_NAME: YOUR_DOCKERHUB_USERNAME/rustassistant  # Change this!
```

In `docker-compose.prod.yml`, update:
```yaml
services:
    rustassistant-web:
        image: YOUR_DOCKERHUB_USERNAME/rustassistant:latest  # Change this!
```

### Step 5: Configure Environment Variables

On Raspberry Pi, create `.env` file:
```bash
ssh actions@YOUR_TAILSCALE_IP
cd ~/rustassistant

cat > .env <<EOF
# RustAssistant Environment Configuration
RUST_LOG=info
XAI_API_KEY=your_grok_api_key_here
XAI_BASE_URL=https://api.x.ai/v1
EOF

chmod 600 .env
```

### Step 6: Deploy!

Push to GitHub:
```bash
git add .
git commit -m "Configure CI/CD for production deployment"
git push origin main
```

Watch the deployment:
- GitHub Actions: `https://github.com/YOUR_USERNAME/rustassistant/actions`
- Discord notifications (if configured)

---

## 🔍 Verification Steps

### Check GitHub Actions
- [ ] Test & Lint job passed (green checkmark)
- [ ] Build & Push job completed
- [ ] Deploy job completed
- [ ] No error messages in logs

### Check Raspberry Pi
SSH to Raspberry Pi:
```bash
ssh -p 22 actions@YOUR_TAILSCALE_IP
```

Check containers are running:
```bash
cd ~/rustassistant
docker compose -f docker-compose.prod.yml ps
```

Expected output:
```
NAME                   STATUS        PORTS
rustassistant-web      Up (healthy)  0.0.0.0:3001->3001/tcp
rustassistant-redis    Up (healthy)  0.0.0.0:6379->6379/tcp
```

Check logs:
```bash
docker compose -f docker-compose.prod.yml logs -f
```

### Test Web UI
From any device on your Tailscale network:
```
http://YOUR_TAILSCALE_IP:3001
```

You should see the RustAssistant dashboard!

### Verify Architecture
Confirm ARM64 image is running:
```bash
docker inspect rustassistant-web | grep Architecture
```

Should show: `"Architecture": "arm64"`

---

## 🐛 Troubleshooting

### Deployment Fails - SSH Connection
**Error:** "Connection refused" or "Host key verification failed"

**Fix:**
```bash
# On Raspberry Pi, verify actions user exists
id actions

# Verify SSH is running
sudo systemctl status ssh

# Verify Tailscale is connected
sudo tailscale status

# Test SSH locally
ssh actions@localhost
```

### Deployment Fails - Docker Pull
**Error:** "pull access denied" or "manifest unknown"

**Fixes:**
1. Verify image name matches in workflow and compose file
2. Check Docker Hub repository exists
3. Verify image was pushed: `docker pull YOUR_USERNAME/rustassistant:latest`
4. Check Docker Hub token has push permissions

### Containers Not Healthy
**Error:** Container status shows "unhealthy" or keeps restarting

**Debug:**
```bash
# Check logs
docker compose -f docker-compose.prod.yml logs rustassistant-web

# Check resource usage
docker stats

# Check disk space
df -h

# Check memory
free -h
```

### Out of Disk Space on Raspberry Pi
**Fix:**
```bash
# Remove old Docker images
docker system prune -a --volumes

# Check what's using space
du -sh /var/lib/docker/*

# Clean Docker build cache
docker builder prune -a
```

### Web UI Not Accessible
**Fixes:**
1. Verify Tailscale is connected on client: `tailscale status`
2. Check firewall on Pi: `sudo ufw status`
3. Verify container is listening: `docker compose -f docker-compose.prod.yml ps`
4. Check logs: `docker compose -f docker-compose.prod.yml logs rustassistant-web`
5. Test locally on Pi: `curl http://localhost:3001`

### GitHub Actions - Tailscale Connection Fails
**Error:** "Failed to connect to Tailscale"

**Fixes:**
1. Verify OAuth credentials are correct in GitHub Secrets
2. Check OAuth client permissions in Tailscale admin console
3. Verify target IP is correct: `tailscale ip -4` on Pi
4. Check if Tailscale service is running on Pi

---

## 🔄 Updating the Deployment

### Regular Updates
Just push to main branch:
```bash
git add .
git commit -m "Update: your changes"
git push origin main
```

CI/CD will automatically:
1. Run tests
2. Build new Docker image
3. Push to Docker Hub
4. Deploy to Raspberry Pi
5. Notify via Discord

### Manual Deployment
SSH to Raspberry Pi:
```bash
ssh actions@YOUR_TAILSCALE_IP
cd ~/rustassistant

# Pull latest images
docker compose -f docker-compose.prod.yml pull

# Restart services
docker compose -f docker-compose.prod.yml up -d

# Check logs
docker compose -f docker-compose.prod.yml logs -f
```

### Rollback to Previous Version
```bash
ssh actions@YOUR_TAILSCALE_IP
cd ~/rustassistant

# Stop current version
docker compose -f docker-compose.prod.yml down

# Pull specific version (if tagged)
docker pull YOUR_USERNAME/rustassistant:PREVIOUS_TAG

# Update compose file with tag, then start
docker compose -f docker-compose.prod.yml up -d
```

---

## 🔐 Security Best Practices

### Completed by Setup
- ✅ SSH key-only authentication (no passwords)
- ✅ Tailscale VPN (no public SSH exposure)
- ✅ UFW firewall configured
- ✅ Docker user permissions separated
- ✅ Secrets stored in GitHub Secrets (encrypted)
- ✅ Container resource limits

### Ongoing Maintenance
- [ ] Rotate SSH keys every 90 days
- [ ] Rotate API keys every 90 days
- [ ] Update Raspberry Pi OS monthly: `sudo apt update && sudo apt upgrade`
- [ ] Update Docker images weekly (automatic via CI/CD)
- [ ] Monitor disk space weekly
- [ ] Review container logs weekly
- [ ] Backup database monthly
- [ ] Test disaster recovery quarterly

### Security Monitoring
```bash
# Check failed SSH attempts
sudo grep "Failed password" /var/log/auth.log

# Check UFW logs
sudo tail -f /var/log/ufw.log

# Check Docker container security
docker scout cves YOUR_USERNAME/rustassistant:latest

# Check for updates
sudo apt list --upgradable
```

---

## 📊 Monitoring

### Container Health
```bash
# Quick status
docker compose -f docker-compose.prod.yml ps

# Resource usage
docker stats

# Detailed inspect
docker inspect rustassistant-web
```

### Logs
```bash
# Follow all logs
docker compose -f docker-compose.prod.yml logs -f

# Specific service
docker compose -f docker-compose.prod.yml logs -f rustassistant-web

# Last 100 lines
docker compose -f docker-compose.prod.yml logs --tail=100
```

### System Resources
```bash
# CPU and memory
htop

# Disk usage
df -h
du -sh ~/rustassistant/*

# Network
sudo tailscale status
ip addr show tailscale0
```

### Database
```bash
# Check database size
du -h ~/rustassistant/data/rustassistant.db

# Backup database
cp ~/rustassistant/data/rustassistant.db ~/rustassistant/data/rustassistant.db.backup-$(date +%Y%m%d)
```

---

## 🆘 Emergency Procedures

### Complete System Failure
1. **SSH to Raspberry Pi via local network** (if Tailscale is down)
2. **Check if Pi is responsive:** `ping RASPBERRY_PI_IP`
3. **Power cycle if unresponsive:** Unplug/replug power
4. **After reboot, verify Tailscale:** `sudo tailscale up`
5. **Check containers:** `cd ~/rustassistant && docker compose -f docker-compose.prod.yml ps`
6. **Review logs:** `docker compose -f docker-compose.prod.yml logs`

### Database Corruption
```bash
# Stop services
docker compose -f docker-compose.prod.yml down

# Restore from backup
cp ~/rustassistant/data/rustassistant.db.backup-YYYYMMDD ~/rustassistant/data/rustassistant.db

# Restart services
docker compose -f docker-compose.prod.yml up -d
```

### Tailscale Connection Lost
```bash
# Reconnect Tailscale
sudo tailscale up

# Verify connection
sudo tailscale status

# Get IP
tailscale ip -4
```

### Out of Disk Space - Critical
```bash
# Emergency cleanup
docker system prune -a --volumes -f
sudo apt clean
sudo apt autoremove -y

# Remove old logs
sudo journalctl --vacuum-time=7d

# Check largest files
sudo du -ah / | sort -rh | head -20
```

---

## 📞 Support & Resources

### Documentation
- Main README: `README.md`
- CI/CD Review: `docs/CICD_REVIEW.md`
- Grok 4.1 Migration: `GROK_4.1_MIGRATION.md`
- Project Status: `docs/STATUS.md`

### Logs Locations
- **GitHub Actions:** `https://github.com/YOUR_USERNAME/rustassistant/actions`
- **Docker Logs:** `docker compose -f docker-compose.prod.yml logs`
- **System Logs:** `/var/log/syslog`
- **SSH Logs:** `/var/log/auth.log`
- **UFW Logs:** `/var/log/ufw.log`

### Quick Commands Reference
```bash
# Status check
docker compose -f docker-compose.prod.yml ps

# View logs
docker compose -f docker-compose.prod.yml logs -f

# Restart service
docker compose -f docker-compose.prod.yml restart rustassistant-web

# Stop all
docker compose -f docker-compose.prod.yml down

# Start all
docker compose -f docker-compose.prod.yml up -d

# Update from Docker Hub
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d

# Clean old images
docker system prune -a

# SSH to Pi
ssh actions@YOUR_TAILSCALE_IP
```

### Getting Help
1. Check GitHub Actions logs for deployment issues
2. Check container logs for runtime issues
3. Check Discord notifications for alerts
4. Review this checklist for common issues
5. Check Tailscale admin console for network issues

---

## ✅ Post-Deployment Checklist

- [ ] Web UI accessible via Tailscale IP
- [ ] Containers show "healthy" status
- [ ] Database is being created/updated
- [ ] Redis cache is working
- [ ] Logs show no errors
- [ ] Discord notifications received (if configured)
- [ ] Can access from multiple Tailscale devices
- [ ] Test note creation/retrieval
- [ ] Test repository analysis (if applicable)
- [ ] Monitor resource usage (< 80% memory, < 90% disk)
- [ ] Credentials file deleted from Raspberry Pi
- [ ] GitHub Secrets verified
- [ ] Backup strategy documented
- [ ] Monitoring schedule established

---

## 🎉 Success!

Your RustAssistant is now deployed and running on Raspberry Pi! 

**Access your deployment:**
- Web UI: `http://YOUR_TAILSCALE_IP:3001`
- From any device on your Tailscale network
- Secure, private, and accessible anywhere

**Next steps:**
1. Set up regular backups
2. Monitor resource usage
3. Schedule secret rotation
4. Enjoy your AI-powered development assistant!

---

**Last Updated:** 2026-02-01  
**Version:** 1.0  
**Deployment Target:** Raspberry Pi (ARM64) via Tailscale