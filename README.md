# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Diagnostico Npcap

A pagina Captura serve para validar se o backend Rust do Tauri consegue acessar o Npcap e receber pacotes de forma passiva. Esta etapa e somente diagnostica: o app contabiliza pacotes e bytes, mas nao armazena payloads, nao envia payloads, nao grava PCAP e nao faz requisicoes externas.

Requisitos:

- Npcap instalado no Windows.
- Para compilar no Windows com a crate `pcap`, tambem e necessaria a biblioteca de importacao `wpcap.lib`, fornecida pelo SDK do Npcap/WinPcap.
- Permissao local para abrir a interface de rede escolhida.
- Em algumas configuracoes do Npcap, talvez seja necessario executar o aplicativo como administrador.

Compilacao no Windows:

- Se `cargo test`, `cargo build` ou `npm run tauri dev` falhar com `cannot open input file 'wpcap.lib'`, o runtime do Npcap pode estar instalado, mas o SDK/import library nao esta disponivel para o linker.
- Instale/extraia manualmente o SDK do Npcap/WinPcap conforme sua preferencia e aponte a pasta que contem `wpcap.lib`.
- O `build.rs` reconhece a variavel `NPCAP_SDK_LIB`. Exemplo: `set NPCAP_SDK_LIB=C:\Npcap-sdk\Lib\x64`.
- Caminhos comuns como `C:\Npcap-sdk\Lib\x64` e `C:\WpdPack\Lib\x64` tambem sao detectados automaticamente quando existem.
- Se o SDK estiver em `C:\Npcap-SDK`, inclua `C:\Npcap-SDK\Lib\x64` na variavel `LIB` durante a compilacao, ou defina `NPCAP_SDK_LIB=C:\Npcap-SDK\Lib\x64`.

Como testar:

1. Execute o app em desenvolvimento com `npm run tauri dev`.
2. Abra a pagina `Captura` no menu lateral.
3. Clique em `Atualizar interfaces`.
4. Escolha uma interface. A interface fisica com IPv4 ativo e nao loopback aparece marcada como sugerida quando possivel.
5. Mantenha o filtro `udp` ou altere/remova o filtro BPF.
6. Clique em `Iniciar`.
7. Gere trafego local abrindo paginas, usando o jogo ou qualquer aplicacao de rede.
8. Observe os contadores de pacotes, bytes, pacotes por segundo e bytes por segundo.
9. Use o painel `Fluxos observados` para filtrar UDP/TCP, pesquisar IP ou porta, ordenar por bytes, pacotes ou atividade recente e acompanhar quais fluxos ficaram ativos nos ultimos segundos.
10. Use `Adicionar marcador` para registrar eventos manuais da sessao, como Albion aberto, troca de mapa, mercado aberto, painel K aberto ou bau aberto.
11. Clique em `Parar` para encerrar a captura.

Sessoes e fluxos:

- Cada clique em `Iniciar` cria uma nova sessao em memoria.
- Os contadores e fluxos sao zerados ao iniciar nova sessao.
- Fluxos sao agregados por protocolo, IP/porta de origem e IP/porta de destino.
- O app analisa apenas cabecalhos Ethernet, IPv4, IPv6, UDP e TCP.
- Pacotes desconhecidos ou truncados incrementam o contador de nao classificados.
- A direcao entrada/saida e inferida pelos enderecos da interface selecionada. Nao ha leitura de processos, memoria ou protocolo do Albion.

Problemas comuns:

- `Npcap indisponivel`: instale o Npcap manualmente e abra o app novamente.
- `DLL indisponivel`: verifique se o Npcap esta instalado corretamente e se `Packet.dll`/`wpcap.dll` estao acessiveis pelo sistema.
- `cannot open input file 'wpcap.lib'`: falta o SDK/import library para o linker, mesmo que `wpcap.dll` exista no Windows.
- `Falta de permissao`: execute o app como administrador se a configuracao local exigir.
- `Filtro BPF invalido`: revise o filtro digitado, por exemplo `udp`, `tcp` ou vazio.
- `Nenhuma interface`: confirme se ha adaptadores ativos e se o Npcap foi instalado com suporte a captura.
