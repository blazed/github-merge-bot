# GitHub Merge Bot

A robust GitHub bot written in Rust that helps ensure PRs are safe to merge by creating try-merge branches and running automated checks.

## Features

- **Multi-repository support**: Handle webhooks from multiple GitHub repositories
- **Stateful operations**: Uses CockroachDB for persistent state management
- **Concurrent webhook handling**: Processes webhooks concurrently while ensuring command serialization to avoid race conditions
- **Try-merge functionality**: Creates temporary branches to test PR mergeability
- **Command-driven**: Responds to PR comments with @bot commands
- **Webhook verification**: Validates GitHub webhook signatures for security

## Commands

- `@bot try` - Creates a try-merge branch at `automation/bot/try/{pr_number}`
- `@bot try-merge` - Creates a try-merge branch at `automation/bot/try-merge/{pr_number}`

## Architecture

The bot is designed with the following components:

- **Webhook Handler**: Receives and validates GitHub webhooks
- **Command Processor**: Parses commands from PR comments
- **GitHub Client**: Interfaces with GitHub API for repository operations
- **Database Layer**: Manages state in CockroachDB
- **Job Queue**: Handles try-merge operations with proper concurrency control

## Setup

### Prerequisites

- Rust 1.75+
- Docker and Docker Compose
- GitHub App or Personal Access Token
- Webhook secret

### Environment Variables

```bash
GITHUB_TOKEN=your_github_token
WEBHOOK_SECRET=your_webhook_secret
DATABASE_URL=postgresql://github_bot:password@localhost:26257/github_bot
BIND_ADDRESS=0.0.0.0:3000
BOT_NAME=bot
RUST_LOG=info
```

### Running with Docker Compose

1. Clone the repository
2. Create a `.env` file with your environment variables:
   ```bash
   GITHUB_TOKEN=ghp_your_token_here
   WEBHOOK_SECRET=your_webhook_secret_here
   ```
3. Start the services:
   ```bash
   docker-compose up -d
   ```

### Running Locally

1. Install CockroachDB locally or use the Docker container
2. Set environment variables
3. Run the bot:
   ```bash
   cargo run
   ```

## GitHub Setup

### Creating a GitHub App

1. Go to GitHub Settings > Developer settings > GitHub Apps
2. Click "New GitHub App"
3. Fill in the required information:
   - **Webhook URL**: `https://your-domain.com/webhook`
   - **Webhook secret**: Use the same value as `WEBHOOK_SECRET`
4. Set permissions:
   - **Repository permissions**:
     - Contents: Read & Write
     - Issues: Read & Write
     - Pull requests: Read & Write
     - Metadata: Read
   - **Subscribe to events**:
     - Issue comments
     - Pull requests
5. Install the app on your repositories

### Webhook Configuration

The bot listens for webhooks at `/webhook` endpoint. Configure your GitHub repository webhooks to point to:
```
https://your-domain.com/webhook
```

## Database Schema

The bot creates two main tables:

- `repositories`: Stores repository information
- `try_merge_jobs`: Tracks try-merge job status and history

## API Endpoints

- `POST /webhook` - GitHub webhook endpoint
- `GET /health` - Health check endpoint

## How It Works

1. **Webhook Reception**: GitHub sends webhooks for PR comments and PR events
2. **Command Parsing**: Bot parses commands from PR comments mentioning @bot
3. **Job Creation**: Creates a try-merge job in the database
4. **Branch Operations**: 
   - Creates a new try branch from the base branch
   - Merges the PR branch into the try branch
   - Monitors CI status
5. **State Management**: Updates job status in the database
6. **Cleanup**: Removes completed jobs from active job tracking

## Concurrency Model

- **Webhook Processing**: Multiple webhooks are processed concurrently
- **Command Execution**: Commands are serialized per repository to avoid race conditions
- **Job Management**: Active jobs are tracked in memory with database persistence
- **Database Operations**: Uses connection pooling for efficient database access

## Error Handling

- Webhook signature validation prevents unauthorized requests
- Database transaction rollbacks ensure data consistency
- GitHub API errors are logged and reported
- Failed jobs are marked with error messages for debugging

## Development

### Project Structure

```
src/
├── main.rs           # Main application and request handlers
├── config.rs         # Configuration management
├── database.rs       # Database operations
├── github.rs         # GitHub API client
├── webhook.rs        # Webhook signature verification
└── commands.rs       # Command parsing logic
```

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
```

### Logging

The bot uses structured logging with the `tracing` crate. Set `RUST_LOG=debug` for detailed logs.

## Security Considerations

- Webhook signatures are verified using HMAC-SHA256
- Database connections use SSL in production
- GitHub tokens should have minimal required permissions
- Bot runs as non-root user in Docker container

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see LICENSE file for details
