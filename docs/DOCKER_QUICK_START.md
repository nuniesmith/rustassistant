# Docker Quick Start - RustAssistant

**Get RustAssistant running with Docker in 5 minutes!**

---

## ⚡ Quick Start (3 Commands)

```bash
# 1. Set your API key
echo "XAI_API_KEY=xai-your-key-here" > .env

# 2. Start services
docker compose up -d

# 3. Open browser
open http://localhost:3001
```

**Done!** 🎉

---

## 📦 What's Running

| Service | Port | Purpose |
|---------|------|---------|
| **Web UI** | 3001 | Dashboard, notes, analysis |
| **Redis** | 6379 | LLM response cache (400x speedup!) |

---

## 🔧 Common Commands

### Start/Stop
```bash
# Start all services
docker compose up -d

# Stop all services
docker compose down

# Restart web UI
docker compose restart rustassistant-web

# View logs
docker compose logs -f rustassistant-web
```

### CLI Operations
```bash
# Run any CLI command
docker compose run --rm rustassistant-cli rustassistant <command>

# Examples:
docker compose run --rm rustassistant-cli rustassistant repo add /app/repos/my-project
docker compose run --rm rustassistant-cli rustassistant analyze batch src/**/*.rs
docker compose run --rm rustassistant-cli rustassistant cache stats
docker compose run --rm rustassistant-cli rustassistant review files src/main.rs
```

### Monitoring
```bash
# Check service status
docker compose ps

# View logs (all services)
docker compose logs -f

# Check Redis cache
docker exec rustassistant-redis redis-cli INFO stats

# Resource usage
docker stats
```

### Maintenance
```bash
# Backup database
cp data/rustassistant.db data/backup-$(date +%Y%m%d).db

# Clear Redis cache
docker exec rustassistant-redis redis-cli FLUSHALL

# Update and restart
docker compose pull
docker compose up -d --force-recreate

# Clean up unused images
docker system prune -a
```

---

## 🍓 Raspberry Pi 4 Setup

```bash
# 1. Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# 2. Clone and configure
git clone https://github.com/jordanistan/rustassistant.git
cd rustassistant
echo "XAI_API_KEY=xai-your-key-here" > .env

# 3. Build and run (takes ~10-15 minutes first time)
docker compose build
docker compose up -d

# 4. Check it's running
docker compose ps
curl http://localhost:3001
```

**Pro Tip**: Use an SSD instead of SD card for much better performance!

---

## 🐛 Troubleshooting

### Web UI won't start?
```bash
# Check logs
docker compose logs rustassistant-web

# Check API key is set
docker compose exec rustassistant-web env | grep XAI_API_KEY

# Restart
docker compose restart rustassistant-web
```

### Port already in use?
```bash
# Find what's using port 3001
sudo lsof -i :3001

# Or change port in docker-compose.yml:
# ports:
#   - "8080:3001"
```

### Redis connection issues?
```bash
# Check Redis is running
docker compose ps redis

# Test connection
docker exec rustassistant-redis redis-cli ping
# Should return: PONG
```

### Out of memory on Pi 4?
```bash
# Reduce Redis memory limit in docker-compose.yml:
# --maxmemory 256mb

# Add swap space
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

---

## 📊 Performance Tips

### Cache Warming (Recommended)
```bash
# After first start, build cache by analyzing your code
docker compose run --rm rustassistant-cli rustassistant analyze batch src/**/*.rs

# Check cache hit rate
docker compose run --rm rustassistant-cli rustassistant cache stats

# Within 24 hours, you should see 70%+ hit rate = 400x speedup!
```

### Resource Limits
```yaml
# For 4GB Pi 4, use these limits in docker-compose.yml:
redis:
  deploy:
    resources:
      limits:
        memory: 256M  # Adjust based on available RAM
```

---

## 🔒 Security Notes

**For Local Use (Default):**
- ✅ Runs on localhost only
- ✅ Non-root containers
- ✅ Read-only template mounts

**For Public Access:**
- Use reverse proxy (nginx/Traefik)
- Enable HTTPS
- Add authentication
- Don't expose Redis port publicly

---

## 📁 Directory Structure

```
rustassistant/
├── docker-compose.yml      # Service definitions
├── docker/
│   ├── Dockerfile.web     # Web UI image
│   └── Dockerfile.cli     # CLI image
├── data/                  # SQLite databases (auto-created)
├── templates/             # HTML templates
├── repos/                 # Your repositories (optional)
└── .env                   # API keys (create this!)
```

---

## 🎯 Quick Reference

| Task | Command |
|------|---------|
| Start | `docker compose up -d` |
| Stop | `docker compose down` |
| Logs | `docker compose logs -f` |
| CLI | `docker compose run --rm rustassistant-cli rustassistant <cmd>` |
| Backup | `cp data/*.db backup/` |
| Update | `docker compose pull && docker compose up -d` |
| Clean | `docker system prune -a` |

---

## 💡 Next Steps

1. **Build your cache**: `docker compose run --rm rustassistant-cli rustassistant analyze batch src/**/*.rs`
2. **Add repositories**: Via Web UI at http://localhost:3001/repos or CLI
3. **Monitor costs**: Check dashboard at http://localhost:3001/costs
4. **Review code**: `docker compose run --rm rustassistant-cli rustassistant review diff`

---

## 📚 Full Documentation

- **Web UI Guide**: `docs/WEB_UI_GUIDE.md`
- **Docker Deployment**: `docs/DOCKER_DEPLOYMENT.md`
- **System Verification**: `SYSTEM_VERIFICATION.md`

---

**Status**: Ready to use! 🚀  
**Web UI**: http://localhost:3001  
**Cache Speedup**: 400x with Redis  
**Monthly Cost**: <$50 with caching  
