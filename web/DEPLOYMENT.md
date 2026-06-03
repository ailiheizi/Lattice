# NextIM Web Client - Deployment Guide

## Overview

This guide covers deploying the NextIM Web Client in various environments.

## Prerequisites

- NextIM Store node running (WebSocket on port 9100, REST API on port 9101)
- Web server (Apache, Nginx, or Python HTTP server)
- Modern web browser (Chrome 90+, Firefox 88+, Safari 14+)

## Deployment Options

### Option 1: Direct File Access (Development)

Simplest method for local development and testing.

```bash
# Open directly in browser
file:///path/to/NextIM/web/index.html
```

**Pros:**
- No server setup required
- Instant changes
- Easy debugging

**Cons:**
- CORS restrictions may apply
- No server-side features
- Not suitable for production

### Option 2: Python HTTP Server (Quick Testing)

Built-in Python server for quick testing.

```bash
# Navigate to web directory
cd /path/to/NextIM/web

# Start server (Python 3)
python -m http.server 8080

# Or use npm script
npm start

# Access at http://localhost:8080
```

**Pros:**
- Quick setup
- No configuration needed
- Good for testing

**Cons:**
- Single-threaded
- No caching
- Not production-ready

### Option 3: Apache HTTP Server (Production)

Full-featured web server with .htaccess support.

#### Installation

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install apache2
sudo systemctl start apache2
sudo systemctl enable apache2
```

**CentOS/RHEL:**
```bash
sudo yum install httpd
sudo systemctl start httpd
sudo systemctl enable httpd
```

#### Configuration

1. Copy web files to Apache document root:
```bash
sudo cp -r /path/to/NextIM/web /var/www/html/nextim
```

2. Enable required modules:
```bash
sudo a2enmod headers
sudo a2enmod expires
sudo a2enmod deflate
sudo a2enmod rewrite
sudo systemctl restart apache2
```

3. The included `.htaccess` file will handle:
   - CORS headers
   - Compression
   - Caching
   - Security headers
   - Directory index

4. Access at `http://your-server/nextim/`

**Pros:**
- Production-ready
- .htaccess support
- Wide compatibility
- Easy configuration

**Cons:**
- Higher resource usage
- More complex setup

### Option 4: Nginx (Production - Recommended)

High-performance web server with reverse proxy support.

#### Installation

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install nginx
sudo systemctl start nginx
sudo systemctl enable nginx
```

**CentOS/RHEL:**
```bash
sudo yum install nginx
sudo systemctl start nginx
sudo systemctl enable nginx
```

#### Configuration

1. Copy web files:
```bash
sudo mkdir -p /var/www/nextim
sudo cp -r /path/to/NextIM/web/* /var/www/nextim/
```

2. Use the included `nginx.conf`:
```bash
sudo cp /path/to/NextIM/web/nginx.conf /etc/nginx/sites-available/nextim
sudo ln -s /etc/nginx/sites-available/nextim /etc/nginx/sites-enabled/
```

3. Update paths in nginx.conf:
```nginx
root /var/www/nextim;
```

4. Test and reload:
```bash
sudo nginx -t
sudo systemctl reload nginx
```

5. Access at `http://your-server/`

**Pros:**
- High performance
- Low resource usage
- Reverse proxy support
- WebSocket support
- Production-ready

**Cons:**
- More complex configuration
- No .htaccess support

### Option 5: Docker (Containerized)

Run in a Docker container for easy deployment.

#### Dockerfile

Create `Dockerfile` in web directory:

```dockerfile
FROM nginx:alpine

# Copy web files
COPY . /usr/share/nginx/html

# Copy nginx config
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Expose port
EXPOSE 80

# Start nginx
CMD ["nginx", "-g", "daemon off;"]
```

#### Build and Run

```bash
# Build image
docker build -t nextim-web:latest .

# Run container
docker run -d \
  --name nextim-web \
  -p 8080:80 \
  nextim-web:latest

# Access at http://localhost:8080
```

**Pros:**
- Isolated environment
- Easy deployment
- Reproducible builds
- Portable

**Cons:**
- Requires Docker
- Additional overhead

## Configuration

### Store Connection

Update Store API URL in the web interface:

1. Open the web client
2. In the sidebar, enter Store API URL:
   - Local: `http://127.0.0.1:9101`
   - Remote: `http://your-store-server:9101`
3. Enter your username
4. Click "Connect"

### Environment Variables

For production deployments, consider using environment variables:

```javascript
// In index.html, replace hardcoded URLs with:
const API_URL = window.ENV?.API_URL || 'http://127.0.0.1:9101';
const WS_URL = window.ENV?.WS_URL || 'ws://127.0.0.1:9100';
```

## Security Considerations

### HTTPS/WSS

For production, always use HTTPS and WSS:

```nginx
server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # ... rest of config
}
```

### Content Security Policy

Add CSP header:

```nginx
add_header Content-Security-Policy "default-src 'self'; connect-src 'self' ws: wss: http: https:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline';" always;
```

### CORS Configuration

For cross-origin requests, configure CORS properly:

```nginx
# Allow specific origins only
add_header Access-Control-Allow-Origin "https://your-domain.com" always;
```

## Performance Optimization

### Enable Compression

**Apache (.htaccess):**
```apache
<IfModule mod_deflate.c>
    AddOutputFilterByType DEFLATE text/html text/css text/javascript application/javascript
</IfModule>
```

**Nginx:**
```nginx
gzip on;
gzip_types text/plain text/css text/javascript application/javascript;
```

### Browser Caching

**Apache:**
```apache
<IfModule mod_expires.c>
    ExpiresActive On
    ExpiresByType text/html "access plus 1 hour"
    ExpiresByType text/css "access plus 1 week"
</IfModule>
```

**Nginx:**
```nginx
location ~* \.(css|js)$ {
    expires 1w;
}
```

## Monitoring

### Access Logs

**Apache:**
```bash
tail -f /var/log/apache2/access.log
```

**Nginx:**
```bash
tail -f /var/log/nginx/access.log
```

### Error Logs

**Apache:**
```bash
tail -f /var/log/apache2/error.log
```

**Nginx:**
```bash
tail -f /var/log/nginx/error.log
```

### Health Checks

Create a health check endpoint:

```bash
# Check if web server is responding
curl -I http://localhost/

# Check Store API
curl http://localhost:9101/health

# Check WebSocket
wscat -c ws://localhost:9100
```

## Troubleshooting

### Web Client Not Loading

1. Check web server is running
2. Check file permissions
3. Check browser console for errors

### Cannot Connect to Store

1. Verify Store is running
2. Check CORS headers
3. Check firewall rules

### WebSocket Connection Failed

1. Check WebSocket port
2. Test WebSocket connection
3. Check proxy configuration

## Production Checklist

- [ ] HTTPS/WSS enabled
- [ ] Security headers configured
- [ ] CORS properly configured
- [ ] Compression enabled
- [ ] Caching configured
- [ ] Monitoring setup
- [ ] Backup strategy in place
- [ ] Error logging enabled
- [ ] Health checks configured
- [ ] Firewall rules set

## License

MIT OR Apache-2.0
