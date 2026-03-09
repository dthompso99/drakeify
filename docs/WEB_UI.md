# Drakeify Web UI

The Drakeify Web UI is a separate executable that provides a web-based management interface for Drakeify configurations.

## Architecture

The Web UI follows a **sidecar pattern**:
- **Separate binary**: `drakeify-web` runs independently from the main `drakeify` proxy
- **Shared database**: Both services connect to the same database (SQLite or PostgreSQL)
- **Security isolation**: Web UI has its own authentication and doesn't have direct LLM access
- **Independent scaling**: Can be scaled separately in Kubernetes/Docker environments

## Features

- ✅ **LLM Configuration Management**: Full CRUD operations for LLM configurations
- ✅ **Live Updates**: WebSocket-based real-time updates when configurations change
- ✅ **Token Authentication**: Simple Bearer token authentication
- ✅ **Database Agnostic**: Works with both SQLite and PostgreSQL
- ✅ **Modern UI**: Clean, dark-themed interface with responsive design

## Configuration

### Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `DATABASE_URL` | Database connection string | - | Yes |
| `DRAKEIFY_WEB_TOKEN` | Authentication token for API access | - | Yes |

### Example Configuration

```bash
# PostgreSQL
export DATABASE_URL="postgresql://drakeify:password@localhost:5432/drakeify"
export DRAKEIFY_WEB_TOKEN="your-secret-token-here"

# SQLite
export DATABASE_URL="sqlite://drakeify.db"
export DRAKEIFY_WEB_TOKEN="your-secret-token-here"
```

## Running the Web UI

### Local Development

```bash
# Set environment variables
export DATABASE_URL="sqlite://drakeify.db"
export DRAKEIFY_WEB_TOKEN="dev-token-123"

# Run the web UI
cargo run --bin drakeify-web
```

The Web UI will be available at `http://localhost:3974`

### Docker Compose

The Web UI is included in the `docker-compose.yml` as a sidecar service:

```bash
# Start all services (proxy + web UI + PostgreSQL)
docker-compose up -d

# View logs
docker-compose logs -f drakeify-web

# Stop services
docker-compose down
```

Access the Web UI at `http://localhost:3974`

### Kubernetes

Example deployment manifest:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: drakeify-web
spec:
  replicas: 1
  selector:
    matchLabels:
      app: drakeify-web
  template:
    metadata:
      labels:
        app: drakeify-web
    spec:
      containers:
      - name: drakeify-web
        image: drakeify-web:latest
        ports:
        - containerPort: 3974
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: drakeify-secrets
              key: database-url
        - name: DRAKEIFY_WEB_TOKEN
          valueFrom:
            secretKeyRef:
              name: drakeify-secrets
              key: web-token
---
apiVersion: v1
kind: Service
metadata:
  name: drakeify-web
spec:
  selector:
    app: drakeify-web
  ports:
  - port: 3974
    targetPort: 3974
```

## API Endpoints

### Authentication

All API endpoints (except `/health` and `/`) require Bearer token authentication:

```bash
curl -H "Authorization: Bearer your-token-here" \
  http://localhost:3974/api/llm/configs
```

### Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check (no auth) |
| `GET` | `/` | Web UI index page (no auth) |
| `GET` | `/ws` | WebSocket for live updates (auth required) |
| `GET` | `/api/llm/configs` | List all LLM configurations |
| `POST` | `/api/llm/configs` | Create new LLM configuration |
| `GET` | `/api/llm/configs/:id` | Get specific configuration |
| `PUT` | `/api/llm/configs/:id` | Update configuration |
| `DELETE` | `/api/llm/configs/:id` | Delete configuration |

## Testing

Run the test script to verify all API endpoints:

```bash
# Set your token
export DRAKEIFY_WEB_TOKEN="your-token-here"

# Run tests
./test-web-api.sh
```

## Security Considerations

1. **Token Storage**: The Web UI stores the authentication token in `localStorage`. For production, consider using secure cookies or session management.

2. **HTTPS**: Always use HTTPS in production. The Web UI doesn't enforce HTTPS, so use a reverse proxy (nginx, Traefik, etc.) to terminate TLS.

3. **Token Rotation**: Implement token rotation for production deployments.

4. **Network Isolation**: In Kubernetes, use NetworkPolicies to restrict access to the Web UI.

## Troubleshooting

### Web UI won't start

Check that:
- `DATABASE_URL` is set correctly
- `DRAKEIFY_WEB_TOKEN` is set
- Port 3974 is not already in use
- Database is accessible and migrations have run

### Authentication fails

- Verify the token matches between server and client
- Check browser console for errors
- Clear `localStorage` and try again

### WebSocket disconnects

- Check firewall rules allow WebSocket connections
- Verify reverse proxy (if any) supports WebSocket upgrades
- Check server logs for connection errors

## Future Enhancements

- [ ] User management and role-based access control
- [ ] Session viewer and management
- [ ] Plugin configuration UI
- [ ] Secrets management UI
- [ ] Metrics and monitoring dashboard
- [ ] Audit logging
- [ ] Multi-factor authentication

