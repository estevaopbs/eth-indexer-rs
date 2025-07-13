# ETH Indexer RS

Um indexador de blockchain Ethereum desenvolvido em Rust, com interface web para explorar os dados indexados.

## Recursos

- ✅ Conexão com nós Ethereum via JSON-RPC
- ✅ Indexação de blocos, transações e logs
- ✅ Banco de dados SQLite local
- ✅ Interface web para exploração dos dados
- ✅ Rastreamento de contas e saldos
- ✅ Detecção de transferências de tokens ERC-20
- ✅ APIs REST para acesso aos dados

## Requisitos

- [Rust](https://www.rust-lang.org/tools/install) 1.54 ou superior
- Um endpoint Ethereum RPC (Infura, Alchemy, ou um nó local)

## Instalação Rápida

```bash
# Clonar o repositório
git clone https://github.com/seu-usuario/eth-indexer-rs.git
cd eth-indexer-rs

# Configurar o projeto
./scripts/setup.sh

# Editar o arquivo .env com seu endpoint Ethereum RPC
# Exemplo:
# ETH_RPC_URL=https://mainnet.infura.io/v3/sua-chave-infura

# Iniciar o indexador
./scripts/start.sh
```

Após iniciar, o aplicativo estará disponível em:

- Interface Web: http://localhost:3000
- API: http://localhost:3000/api

## Configuração

O arquivo `.env` contém todas as configurações necessárias:

```
# Database connection string
DATABASE_URL=sqlite:./data/indexer.db

# Ethereum RPC URL (replace with your own endpoint)
ETH_RPC_URL=https://mainnet.infura.io/v3/your-infura-key

# API server port
API_PORT=3000

# Starting block number (optional, defaults to 0)
# START_BLOCK=15000000

# Maximum concurrent RPC requests
MAX_CONCURRENT_REQUESTS=5

# Number of blocks to process in a batch
BLOCKS_PER_BATCH=10

# Log level: trace, debug, info, warn, error
LOG_LEVEL=info
```

## Estrutura do Banco de Dados

O indexador organiza os dados em várias tabelas:

- `blocks`: Armazena informações de blocos
- `transactions`: Armazena transações
- `logs`: Armazena logs de eventos
- `accounts`: Rastreia contas e saldos
- `token_transfers`: Rastreia transferências de tokens ERC-20

## API Endpoints

### Blocos

- `GET /api/blocks` - Listar blocos (paginado)
- `GET /api/blocks/:number` - Obter bloco específico

### Transações

- `GET /api/transactions` - Listar transações (paginado)
- `GET /api/transactions/:hash` - Obter transação específica

### Contas

- `GET /api/accounts/:address` - Obter informações de conta

### Outros

- `GET /api/health` - Verificação de saúde
- `GET /api/stats` - Estatísticas gerais
- `GET /api/search/:query` - Buscar blocos/transações/contas

## Desenvolvimento

Este projeto utiliza SQLx com modo offline para evitar dependências de banco de dados durante a compilação.

### Compilação

```bash
# Compilação padrão (usa SQLx offline)
./scripts/dev.sh

# Compilação e execução
./scripts/dev.sh --run

# Compilação manual
export SQLX_OFFLINE=true
cargo build
```

### Estrutura de Migrações

As migrações estão localizadas em `./src/database/migrations/` e são executadas automaticamente na inicialização:

- `001__initial_schema.sql` - Schema inicial (blocos, transações, logs)
- `002__add_accounts.sql` - Tabela de contas
- `003__add_token_transfers.sql` - Tabela de transferências de tokens

### Testes

```bash
cargo test
```

### Executando em Modo Debug

```bash
cargo run
```

## Licença

Este projeto é licenciado sob a Licença MIT - veja o arquivo [LICENSE](LICENSE) para mais detalhes.
