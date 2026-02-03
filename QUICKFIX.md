# 🚀 Quick Fix - Database Permissions Issue

**Issue:** Container restarting with "unable to open database file" error  
**Status:** ✅ Easy fix - takes 30 seconds  
**Cause:** Missing `data` directory or wrong permissions

---

## 🎯 The Problem

Your deployment worked! The ARM64 images are running on your Raspberry Pi. However, the container can't create the SQLite database because:

1. The `data` directory doesn't exist, OR
2. The directory exists but has wrong permissions

**Current Status:**
```bash
jordan@rasp:~$ docker ps
CONTAINER ID   IMAGE                             STATUS
df5a1d894890   nuniesmith/rustassistant:latest   Restarting (1) 16 seconds ago
c7f44815bcaf   redis:7-alpine                    Up About a minute (healthy)
```

The container runs as UID 1000 (user `rustassistant`) and needs to write to the `data` directory mounted from the host.

---

## ✅ Quick Fix (Option 1) - Automated Script

**On your Raspberry Pi, run:**

```bash
cd ~/rustassistant

# Download and run the fix script
chmod +x fix-permissions.sh
./fix-permissions.sh
```

This script will:
1. Stop containers
2. Create `data` and `config` directories
3. Set proper permissions (755, owned by your user)
4. Restart containers
5. Show you the status

**That's it!** The container should start successfully.

---

## ✅ Quick Fix (Option 2) - Manual Steps

**If you prefer to do it manually:**

```bash
# 1. Navigate to project directory
cd ~/rustassistant

# 2. Stop containers
docker compose -f docker-compose.prod.yml down

# 3. Create directories with proper permissions
mkdir -p data config
chmod 755 data config
chown -R $(id -u):$(id -g) data config

# 4. Start containers
docker compose -f docker-compose.prod.yml up -d

# 5. Check logs
docker logs rustassistant-web -f
```

---

## 🔍 Verify It's Working

After running the fix, check:

```bash
# Check container status (should show "Up")
docker ps

# Check logs (should show successful startup)
docker logs rustassistant-web

# Test the endpoint
curl http://localhost:3001/

# Or with proper hostname
curl http://localhost:3001/health
```

**Expected Output:**
- Container status: `Up X seconds (healthy)`
- Logs: No more "unable to open database file" errors
- Curl: Should return HTML or JSON response (not connection refused)

---

## 📋 What Should You See

**Before Fix:**
```
Error: Database error: error returned from database: (code: 14) unable to open database file
```

**After Fix:**
```
INFO rustassistant_server: Initializing database at sqlite:data/rustassistant.db
INFO rustassistant_server: Running migrations...
INFO rustassistant_server: Starting server on 0.0.0.0:3001
INFO rustassistant_server: Server started successfully
```

---

## 🔄 Future Deployments

The CI/CD workflow has been updated to automatically create these directories. So this is a **one-time fix**.

After you push the updated workflow (already modified), future deployments will:

1. ✅ Automatically create `data` and `config` directories
2. ✅ Set proper permissions
3. ✅ Start containers successfully

---

## 🐛 Troubleshooting

### Container still restarting?

```bash
# Check detailed logs
docker logs rustassistant-web --tail=50

# Check disk space
df -h

# Check directory permissions
ls -la ~/rustassistant/ | grep data

# Check what user the container runs as
docker exec rustassistant-web id
```

### Permission denied errors?

```bash
# Make sure you own the directories
sudo chown -R $(whoami):$(whoami) ~/rustassistant/data
sudo chown -R $(whoami):$(whoami) ~/rustassistant/config
chmod 755 ~/rustassistant/data
chmod 755 ~/rustassistant/config
```

### Database file created but can't be opened?

```bash
# Check the database file permissions
ls -la ~/rustassistant/data/

# If the file exists with wrong permissions:
rm ~/rustassistant/data/rustassistant*.db
docker compose -f docker-compose.prod.yml restart
```

---

## 📊 Directory Structure

After the fix, your directory should look like this:

```
~/rustassistant/
├── data/                          # SQLite databases (created by container)
│   ├── rustassistant.db          # Main database
│   └── rustassistant_cache.db    # Cache database
├── config/                        # Optional config files
├── docker-compose.prod.yml
├── .env                          # Environment variables
└── fix-permissions.sh            # This fix script
```

**Permissions:**
- `data/`: 755 (rwxr-xr-x) owned by your user
- `config/`: 755 (rwxr-xr-x) owned by your user
- Database files inside `data/`: Created by container (UID 1000)

---

## 🎓 Why This Happened

The container runs as a non-root user for security (UID 1000). When you mount `./data:/app/data`, the container needs:

1. The host directory to exist
2. Permissions to write to it

Since this was the first deployment, the `data` directory didn't exist yet, causing the SQLite error.

---

## 🚀 Next Steps

1. **Run the fix** using either option above
2. **Verify** the container is running: `docker ps`
3. **Test** the application: `curl http://localhost:3001/`
4. **Commit** the workflow changes (already done on your dev machine)
5. **Push** to trigger automatic deployment next time

---

## ✅ Success Checklist

- [ ] `data` directory exists with 755 permissions
- [ ] `config` directory exists with 755 permissions
- [ ] Container shows status "Up" (not "Restarting")
- [ ] No database errors in logs
- [ ] Can access http://localhost:3001/
- [ ] Both containers healthy: `docker compose -f docker-compose.prod.yml ps`

---

**Need help?** Check the logs:
```bash
docker logs rustassistant-web -f
docker logs rustassistant-redis -f
```

**All good?** Commit and push the workflow updates to prevent this in future deployments! 🎉