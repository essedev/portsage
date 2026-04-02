---
name: grimport
description: >
  Gestisce l'allocazione delle porte tra progetti di sviluppo.
  Usa quando devi assegnare porte a un nuovo progetto, registrare servizi,
  o verificare quali porte sono in uso.
---

# Grimport - Port Allocation Manager

Accesso al database locale delle porte tramite l'app Grimport.

## Tool disponibili

### list_all
Mostra tutti i progetti registrati con range porte, servizi e stato attivo.
Usa come primo passo per capire la situazione attuale.

### reserve_range
Riserva il prossimo range di porte libero per un nuovo progetto.
- `project_name`: nome del progetto (es. "my-app")
- `path`: path opzionale alla directory del progetto

### register_port
Registra una porta specifica per un servizio dentro il range di un progetto.
- `project_name`: nome del progetto
- `service`: nome del servizio (es. "vite", "postgres", "redis", "minio")
- `port`: numero porta (deve essere nel range del progetto)

### release_project
Libera il range di porte di un progetto.
- `project_name`: nome del progetto da liberare

### scan_active
Scanna tutte le porte TCP attive sulla macchina.

## Workflow consigliato

Quando assegni porte a un nuovo progetto:
1. Chiama `list_all` per vedere i range occupati
2. Chiama `reserve_range` con il nome del progetto
3. Usa le porte del range assegnato nel docker-compose.yml e vite.config
4. Chiama `register_port` per ogni servizio configurato

Quando verifichi conflitti:
1. Chiama `scan_active` per vedere le porte attive
2. Chiama `list_all` per incrociare con le porte registrate
