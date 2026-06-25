import {
  BarChart3,
  Boxes,
  Calculator,
  Home,
  RadioReceiver,
  Settings,
  Store,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";
import { inventoryStacks } from "./data/mock/inventory";
import { prices } from "./data/mock/prices";
import { recipes } from "./data/mock/recipes";
import { itemsById } from "./domain/catalog";
import {
  analyzeCrafting,
  compareCraftingFocus,
} from "./domain/economy/craftingEngine";
import type { City } from "./domain/types";

type ViewId =
  | "home"
  | "crafting"
  | "opportunities"
  | "market"
  | "inventory"
  | "capture"
  | "settings";

type CaptureDevice = {
  name: string;
  description?: string | null;
  addresses: string[];
  is_loopback: boolean;
  has_ipv4: boolean;
  is_virtual: boolean;
  is_suggested: boolean;
};

type CaptureStatus = {
  available: boolean;
  running: boolean;
  session_id?: string | null;
  selected_device?: string | null;
  filter?: string | null;
  packets_total: number;
  bytes_total: number;
  packets_per_second: number;
  bytes_per_second: number;
  unclassified_packets: number;
  started_at?: string | null;
  stopped_at?: string | null;
  duration_seconds?: number | null;
  last_packet_at?: string | null;
  last_error?: string | null;
};

type NetworkProtocol = "UDP" | "TCP" | "OTHER";
type FlowDirection = "Outbound" | "Inbound" | "Unknown";
type FlowFilter = "ALL" | "UDP" | "TCP";
type FlowSort = "bytes" | "packets" | "recent";

type NetworkFlow = {
  protocol: NetworkProtocol;
  direction: FlowDirection;
  source_ip: string;
  source_port?: number | null;
  destination_ip: string;
  destination_port?: number | null;
  packets: number;
  bytes: number;
  packets_per_second: number;
  bytes_per_second: number;
  first_seen_at: string;
  last_seen_at: string;
  active_now: boolean;
};

type CaptureMarker = {
  session_id: string;
  timestamp: string;
  description: string;
  packets_total: number;
  bytes_total: number;
};

const navItems: Array<{ id: ViewId; label: string; icon: typeof Home }> = [
  { id: "home", label: "Início", icon: Home },
  { id: "crafting", label: "Crafting", icon: Calculator },
  { id: "opportunities", label: "Oportunidades", icon: BarChart3 },
  { id: "market", label: "Mercado", icon: Store },
  { id: "inventory", label: "Inventário", icon: Boxes },
  { id: "capture", label: "Captura", icon: RadioReceiver },
  { id: "settings", label: "Configurações", icon: Settings },
];

const defaultCaptureStatus: CaptureStatus = {
  available: false,
  running: false,
  session_id: null,
  selected_device: null,
  filter: null,
  packets_total: 0,
  bytes_total: 0,
  packets_per_second: 0,
  bytes_per_second: 0,
  unclassified_packets: 0,
  started_at: null,
  stopped_at: null,
  duration_seconds: null,
  last_packet_at: null,
  last_error: null,
};

const defaultSettings = {
  city: "Martlock" as City,
  premiumActive: true,
  focusAvailable: 12_000,
  marketFeeRate: 0.065,
  stationTaxRate: 0.085,
  baseResourceReturnRate: 0.152,
  focusResourceReturnRate: 0.435,
  cityBonusRate: 0.1,
  dailyBonusRate: 0.05,
};

const LAST_DEVICE_KEY = "albion-assistant:last-capture-device";

const silver = new Intl.NumberFormat("pt-BR", {
  maximumFractionDigits: 0,
});

const integer = new Intl.NumberFormat("pt-BR", {
  maximumFractionDigits: 0,
});

function App() {
  const [activeView, setActiveView] = useState<ViewId>("home");
  const [selectedRecipeId, setSelectedRecipeId] = useState(recipes[0].id);
  const [quantity, setQuantity] = useState(10);
  const [useFocus, setUseFocus] = useState(true);
  const [city, setCity] = useState<City>("Martlock");

  const settings = useMemo(() => ({ ...defaultSettings, city }), [city]);
  const selectedRecipe =
    recipes.find((recipe) => recipe.id === selectedRecipeId) ?? recipes[0];

  const focusComparison = useMemo(
    () =>
      compareCraftingFocus({
        recipe: selectedRecipe,
        craftCount: quantity,
        settings,
      }),
    [quantity, selectedRecipe, settings],
  );

  const craftingResult = useFocus
    ? focusComparison.withFocus
    : focusComparison.withoutFocus;

  const opportunities = useMemo(
    () =>
      recipes
        .map((recipe) =>
          analyzeCrafting({
            recipe,
            craftCount: 10,
            settings,
            useFocus: true,
          }),
        )
        .filter((result) => result.economicProfit > 0 && result.roi >= 0.05)
        .sort((a, b) => b.economicProfit - a.economicProfit),
    [settings],
  );

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brandMark">AA</div>
          <div>
            <strong>Albion Assistant</strong>
            <span>Economia e crafting</span>
          </div>
        </div>

        <nav className="nav">
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <button
                className={activeView === item.id ? "active" : ""}
                key={item.id}
                onClick={() => setActiveView(item.id)}
                type="button"
              >
                <Icon size={18} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
      </aside>

      <main className="workspace">
        {activeView === "home" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">Painel</p>
                <h1>Início</h1>
              </div>
              <StatusPill label="Dados simulados" />
            </header>
            <div className="metricGrid">
              <Metric label="Receitas" value={recipes.length.toString()} />
              <Metric
                label="Itens no inventário"
                value={inventoryStacks.length.toString()}
              />
              <Metric label="Preços mock" value={prices.length.toString()} />
              <Metric
                label="Melhor lucro econômico"
                value={`${silver.format(opportunities[0]?.economicProfit ?? 0)} prata`}
              />
            </div>
          </section>
        )}

        {activeView === "crafting" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">Motor econômico</p>
                <h1>Crafting</h1>
              </div>
              <StatusPill label={useFocus ? "Com foco" : "Sem foco"} />
            </header>

            <div className="toolbar">
              <label>
                Item
                <select
                  value={selectedRecipeId}
                  onChange={(event) => setSelectedRecipeId(event.currentTarget.value)}
                >
                  {recipes.map((recipe) => (
                    <option key={recipe.id} value={recipe.id}>
                      {itemsById[recipe.outputItemId].name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Quantidade
                <input
                  min={1}
                  type="number"
                  value={quantity}
                  onChange={(event) =>
                    setQuantity(Math.max(1, Number(event.currentTarget.value)))
                  }
                />
              </label>
              <label>
                Cidade
                <select
                  value={city}
                  onChange={(event) => setCity(event.currentTarget.value as City)}
                >
                  {["Martlock", "Bridgewatch", "Fort Sterling", "Lymhurst", "Thetford"].map(
                    (cityName) => (
                      <option key={cityName}>{cityName}</option>
                    ),
                  )}
                </select>
              </label>
              <label className="toggle">
                <input
                  checked={useFocus}
                  type="checkbox"
                  onChange={(event) => setUseFocus(event.currentTarget.checked)}
                />
                Foco
              </label>
            </div>

            <div className="metricGrid">
              <Metric
                label="Lucro em caixa"
                value={`${silver.format(craftingResult.cashProfit)} prata`}
              />
              <Metric
                label="Lucro econômico"
                value={`${silver.format(craftingResult.economicProfit)} prata`}
              />
              <Metric label="ROI" value={`${(craftingResult.roi * 100).toFixed(1)}%`} />
              <Metric
                label="Lucro por foco"
                value={`${focusComparison.additionalProfitPerFocusPoint.toFixed(2)}`}
              />
            </div>

            <div className="split">
              <Panel title="Materiais">
                <table>
                  <thead>
                    <tr>
                      <th>Item</th>
                      <th>Necessário</th>
                      <th>Possuído</th>
                      <th>Faltante</th>
                      <th>Custo</th>
                    </tr>
                  </thead>
                  <tbody>
                    {craftingResult.materials.map((line) => (
                      <tr key={line.itemId}>
                        <td>{itemsById[line.itemId].name}</td>
                        <td>{line.requiredBeforeReturn}</td>
                        <td>{line.ownedQuantity}</td>
                        <td>{line.missingQuantity}</td>
                        <td>{silver.format(line.cashCost)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </Panel>
              <Panel title="Cálculo">
                <dl className="details">
                  <div>
                    <dt>Receita líquida</dt>
                    <dd>{silver.format(craftingResult.netRevenue)}</dd>
                  </div>
                  <div>
                    <dt>Taxa da estação</dt>
                    <dd>{silver.format(craftingResult.stationFee)}</dd>
                  </div>
                  <div>
                    <dt>Custo em caixa</dt>
                    <dd>{silver.format(craftingResult.totalCashMaterialCost)}</dd>
                  </div>
                  <div>
                    <dt>Custo de oportunidade</dt>
                    <dd>{silver.format(craftingResult.totalOpportunityCost)}</dd>
                  </div>
                </dl>
                <ul className="explain">
                  {craftingResult.explanation.map((line) => (
                    <li key={line}>{line}</li>
                  ))}
                </ul>
              </Panel>
            </div>
          </section>
        )}

        {activeView === "opportunities" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">Análise</p>
                <h1>Oportunidades</h1>
              </div>
              <StatusPill label={`${opportunities.length} resultados`} />
            </header>
            <Panel title="Crafts lucrativos">
              <table>
                <thead>
                  <tr>
                    <th>Item</th>
                    <th>Cidade</th>
                    <th>Lucro econômico</th>
                    <th>ROI</th>
                    <th>Foco</th>
                  </tr>
                </thead>
                <tbody>
                  {opportunities.map((result) => (
                    <tr key={result.recipeId}>
                      <td>{itemsById[result.outputItemId].name}</td>
                      <td>{result.city}</td>
                      <td>{silver.format(result.economicProfit)}</td>
                      <td>{(result.roi * 100).toFixed(1)}%</td>
                      <td>{result.focusCost}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </Panel>
          </section>
        )}

        {activeView === "market" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">Preços</p>
                <h1>Mercado</h1>
              </div>
              <StatusPill label="Mock manual" />
            </header>
            <Panel title="Ordens">
              <table>
                <thead>
                  <tr>
                    <th>Item</th>
                    <th>Cidade</th>
                    <th>Qualidade</th>
                    <th>Venda</th>
                    <th>Compra</th>
                    <th>Qtd.</th>
                    <th>Atualizado</th>
                  </tr>
                </thead>
                <tbody>
                  {prices.map((price) => (
                    <tr key={`${price.itemId}-${price.city}-${price.quality}`}>
                      <td>{itemsById[price.itemId].name}</td>
                      <td>{price.city}</td>
                      <td>{price.quality}</td>
                      <td>{silver.format(price.sellPrice)}</td>
                      <td>{silver.format(price.buyPrice)}</td>
                      <td>
                        {price.sellQuantity}/{price.buyQuantity}
                      </td>
                      <td>{price.updatedAt}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </Panel>
          </section>
        )}

        {activeView === "inventory" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">JSON local</p>
                <h1>Inventário</h1>
              </div>
              <StatusPill label="Simulado" />
            </header>
            <Panel title="Itens possuídos">
              <table>
                <thead>
                  <tr>
                    <th>Item</th>
                    <th>Quantidade</th>
                    <th>Qualidade</th>
                  </tr>
                </thead>
                <tbody>
                  {inventoryStacks.map((stack) => (
                    <tr key={`${stack.itemId}-${stack.quality}`}>
                      <td>{itemsById[stack.itemId].name}</td>
                      <td>{stack.quantity}</td>
                      <td>{stack.quality}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </Panel>
          </section>
        )}

        {activeView === "capture" && <CaptureView />}

        {activeView === "settings" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">Perfil</p>
                <h1>Configurações</h1>
              </div>
              <StatusPill label={settings.premiumActive ? "Premium ativo" : "Sem premium"} />
            </header>
            <div className="metricGrid">
              <Metric label="Foco disponível" value={silver.format(settings.focusAvailable)} />
              <Metric label="Taxa de mercado" value={`${settings.marketFeeRate * 100}%`} />
              <Metric label="Taxa de estação" value={`${settings.stationTaxRate * 100}%`} />
              <Metric
                label="Retorno com foco"
                value={`${(settings.focusResourceReturnRate * 100).toFixed(1)}%`}
              />
            </div>
          </section>
        )}
      </main>
    </div>
  );
}

function CaptureView() {
  const [devices, setDevices] = useState<CaptureDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState("");
  const [filter, setFilter] = useState("udp");
  const [status, setStatus] = useState<CaptureStatus>(defaultCaptureStatus);
  const [flows, setFlows] = useState<NetworkFlow[]>([]);
  const [markers, setMarkers] = useState<CaptureMarker[]>([]);
  const [flowFilter, setFlowFilter] = useState<FlowFilter>("ALL");
  const [flowSearch, setFlowSearch] = useState("");
  const [flowSort, setFlowSort] = useState<FlowSort>("bytes");
  const [flowLimit, setFlowLimit] = useState(50);
  const [message, setMessage] = useState("");

  const recommendedDevice = useMemo(
    () => devices.find((device) => device.is_suggested),
    [devices],
  );

  const selectedDeviceInfo = useMemo(
    () => devices.find((device) => device.name === selectedDevice),
    [devices, selectedDevice],
  );

  const visibleFlows = useMemo(() => {
    const search = flowSearch.trim().toLowerCase();
    const filtered = flows.filter((flow) => {
      const protocolMatches = flowFilter === "ALL" || flow.protocol === flowFilter;
      const searchMatches =
        !search ||
        flow.source_ip.toLowerCase().includes(search) ||
        flow.destination_ip.toLowerCase().includes(search) ||
        String(flow.source_port ?? "").includes(search) ||
        String(flow.destination_port ?? "").includes(search);

      return protocolMatches && searchMatches;
    });

    return filtered.sort((a, b) => {
      if (flowSort === "packets") {
        return b.packets - a.packets;
      }
      if (flowSort === "recent") {
        return Number(b.last_seen_at) - Number(a.last_seen_at);
      }
      return b.bytes - a.bytes;
    });
  }, [flowFilter, flowSearch, flowSort, flows]);

  const refreshStatus = useCallback(async () => {
    const nextStatus = await invoke<CaptureStatus>("capture_get_status");
    setStatus(nextStatus);
    if (nextStatus.running && nextStatus.selected_device) {
      setSelectedDevice(nextStatus.selected_device);
    }
  }, []);

  const refreshFlows = useCallback(async () => {
    const nextFlows = await invoke<NetworkFlow[]>("capture_get_flows", {
      request: { limit: flowLimit },
    });
    setFlows(nextFlows);
  }, [flowLimit]);

  const refreshMarkers = useCallback(async () => {
    const nextMarkers = await invoke<CaptureMarker[]>("capture_get_markers");
    setMarkers(nextMarkers);
  }, []);

  const refreshDevices = useCallback(async () => {
    try {
      const availability = await invoke<CaptureStatus>("capture_check_availability");
      setStatus(availability);
      const nextDevices = await invoke<CaptureDevice[]>("capture_list_devices");
      setDevices(nextDevices);
      setMessage(
        nextDevices.length === 0
          ? "Nenhuma interface foi encontrada pelo Npcap."
          : "Interfaces atualizadas.",
      );

      const savedDevice = window.localStorage.getItem(LAST_DEVICE_KEY);
      const restored = nextDevices.find((device) => device.name === savedDevice);
      const suggested = nextDevices.find((device) => device.is_suggested);
      setSelectedDevice((current) => {
        if (current && nextDevices.some((device) => device.name === current)) {
          return current;
        }
        return restored?.name ?? suggested?.name ?? nextDevices[0]?.name ?? "";
      });
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void refreshDevices();
  }, [refreshDevices]);

  useEffect(() => {
    const interval = window.setInterval(() => {
      void refreshStatus();
      void refreshFlows();
      void refreshMarkers();
    }, 1000);

    return () => window.clearInterval(interval);
  }, [refreshFlows, refreshMarkers, refreshStatus]);

  useEffect(() => {
    if (selectedDevice) {
      window.localStorage.setItem(LAST_DEVICE_KEY, selectedDevice);
    }
  }, [selectedDevice]);

  async function startCapture() {
    try {
      await invoke("capture_start", {
        request: {
          device_name: selectedDevice,
          filter,
        },
      });
      setFlows([]);
      setMarkers([]);
      setMessage("Captura iniciada.");
      await refreshStatus();
      await refreshFlows();
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function stopCapture() {
    try {
      const stopped = await invoke<CaptureStatus>("capture_stop");
      setStatus(stopped);
      setMessage("Captura parada.");
      await refreshFlows();
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function addMarker() {
    const description = window.prompt("Descrição do marcador");
    if (!description) {
      return;
    }

    try {
      const marker = await invoke<CaptureMarker>("capture_add_marker", {
        request: { description },
      });
      setMarkers((current) => [...current, marker]);
      setMessage("Marcador adicionado.");
    } catch (error) {
      setMessage(String(error));
    }
  }

  function clearFlowView() {
    setFlowFilter("ALL");
    setFlowSearch("");
    setFlowSort("bytes");
    setFlowLimit(50);
  }

  return (
    <section className="view captureView">
      <header className="viewHeader">
        <div>
          <p className="eyebrow">Diagnóstico local</p>
          <h1>Captura</h1>
        </div>
        <StatusPill label={status.running ? "Captura ativa" : "Captura parada"} />
      </header>

      <div className="notice">
        A captura é local e passiva. Nesta versão, o aplicativo contabiliza pacotes e
        agrega fluxos, mas não armazena nem envia o conteúdo capturado.
      </div>

      <div className="metricGrid captureMetrics">
        <Metric label="Npcap" value={status.available ? "Disponível" : "Indisponível"} />
        <Metric label="Pacotes" value={integer.format(status.packets_total)} />
        <Metric label="Bytes" value={integer.format(status.bytes_total)} />
        <Metric label="Pacotes/s" value={status.packets_per_second.toFixed(1)} />
        <Metric label="Bytes/s" value={status.bytes_per_second.toFixed(0)} />
        <Metric label="Não classificados" value={integer.format(status.unclassified_packets)} />
      </div>

      <Panel title="Controle">
        <div className="captureControls">
          <label className="wideControl">
            Interface
            <select
              value={selectedDevice}
              onChange={(event) => setSelectedDevice(event.currentTarget.value)}
            >
              <option value="">Selecione</option>
              {devices.map((device) => (
                <option key={device.name} value={device.name}>
                  {deviceLabel(device)}
                </option>
              ))}
            </select>
          </label>
          <label>
            Filtro BPF
            <input value={filter} onChange={(event) => setFilter(event.currentTarget.value)} />
          </label>
          <button type="button" onClick={() => void refreshDevices()}>
            Atualizar interfaces
          </button>
          <button
            disabled={!selectedDevice || status.running}
            type="button"
            onClick={() => void startCapture()}
          >
            Iniciar
          </button>
          <button disabled={!status.running} type="button" onClick={() => void stopCapture()}>
            Parar
          </button>
          <button type="button" onClick={() => void addMarker()}>
            Adicionar marcador
          </button>
        </div>
      </Panel>

      <div className="split">
        <Panel title="Interfaces">
          <div className="tableScroller">
            <table className="interfacesTable">
              <thead>
                <tr>
                  <th>Descrição</th>
                  <th>Nome interno</th>
                  <th>Endereços</th>
                  <th>Tipo</th>
                </tr>
              </thead>
              <tbody>
                {devices.map((device) => (
                  <tr
                    className={[
                      recommendedDevice?.name === device.name ? "recommendedRow" : "",
                      selectedDevice === device.name ? "selectedRow" : "",
                    ].join(" ")}
                    key={device.name}
                  >
                    <td>{device.description ?? "-"}</td>
                    <td>{device.name}</td>
                    <td>{device.addresses.join(", ") || "-"}</td>
                    <td>
                      {device.is_loopback
                        ? "Loopback"
                        : device.is_virtual
                          ? "Virtual"
                          : "Física"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>

        <Panel title="Status da sessão">
          <dl className="details">
            <div>
              <dt>Interface usada</dt>
              <dd>{status.selected_device ?? selectedDeviceInfo?.description ?? "-"}</dd>
            </div>
            <div>
              <dt>Sessão</dt>
              <dd>{status.session_id ?? "-"}</dd>
            </div>
            <div>
              <dt>Filtro</dt>
              <dd>{status.filter ?? "-"}</dd>
            </div>
            <div>
              <dt>Início</dt>
              <dd>{formatTimestamp(status.started_at)}</dd>
            </div>
            <div>
              <dt>Último pacote</dt>
              <dd>{formatTimestamp(status.last_packet_at)}</dd>
            </div>
            <div>
              <dt>Duração</dt>
              <dd>{formatDuration(status.duration_seconds)}</dd>
            </div>
          </dl>
          {(message || status.last_error) && (
            <div className="errorBox">{status.last_error ?? message}</div>
          )}
        </Panel>
      </div>

      <Panel title="Fluxos observados">
        <div className="flowToolbar">
          <label>
            Protocolo
            <select
              value={flowFilter}
              onChange={(event) => setFlowFilter(event.currentTarget.value as FlowFilter)}
            >
              <option value="ALL">Todos</option>
              <option value="UDP">UDP</option>
              <option value="TCP">TCP</option>
            </select>
          </label>
          <label>
            Pesquisar IP ou porta
            <input
              value={flowSearch}
              onChange={(event) => setFlowSearch(event.currentTarget.value)}
            />
          </label>
          <label>
            Ordenar por
            <select
              value={flowSort}
              onChange={(event) => setFlowSort(event.currentTarget.value as FlowSort)}
            >
              <option value="bytes">Bytes</option>
              <option value="packets">Pacotes</option>
              <option value="recent">Atividade recente</option>
            </select>
          </label>
          <label>
            Limite
            <select
              value={flowLimit}
              onChange={(event) => setFlowLimit(Number(event.currentTarget.value))}
            >
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
            </select>
          </label>
          <button type="button" onClick={clearFlowView}>
            Limpar visualização
          </button>
        </div>

        <div className="tableScroller">
          <table className="flowsTable">
            <thead>
              <tr>
                <th>Estado</th>
                <th>Protocolo</th>
                <th>Direção</th>
                <th>IP origem</th>
                <th>Porta origem</th>
                <th>IP destino</th>
                <th>Porta destino</th>
                <th>Pacotes</th>
                <th>Bytes</th>
                <th>Pacotes/s</th>
                <th>Bytes/s</th>
                <th>Último pacote</th>
              </tr>
            </thead>
            <tbody>
              {visibleFlows.map((flow) => (
                <tr className={flow.active_now && status.running ? "activeFlow" : ""} key={flowKey(flow)}>
                  <td>{flowState(flow, status.running)}</td>
                  <td>{flow.protocol}</td>
                  <td>{directionLabel(flow.direction)}</td>
                  <td>{flow.source_ip}</td>
                  <td>{flow.source_port ?? "-"}</td>
                  <td>{flow.destination_ip}</td>
                  <td>{flow.destination_port ?? "-"}</td>
                  <td>{integer.format(flow.packets)}</td>
                  <td>{integer.format(flow.bytes)}</td>
                  <td>{flow.packets_per_second.toFixed(1)}</td>
                  <td>{flow.bytes_per_second.toFixed(0)}</td>
                  <td>{formatTimestamp(flow.last_seen_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>

      <Panel title="Marcadores">
        <div className="tableScroller">
          <table>
            <thead>
              <tr>
                <th>Horário</th>
                <th>Descrição</th>
                <th>Pacotes</th>
                <th>Bytes</th>
              </tr>
            </thead>
            <tbody>
              {markers.map((marker) => (
                <tr key={`${marker.session_id}-${marker.timestamp}-${marker.description}`}>
                  <td>{formatTimestamp(marker.timestamp)}</td>
                  <td>{marker.description}</td>
                  <td>{integer.format(marker.packets_total)}</td>
                  <td>{integer.format(marker.bytes_total)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <article className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}

function Panel({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="panel">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function StatusPill({ label }: { label: string }) {
  return <span className="statusPill">{label}</span>;
}

function deviceLabel(device: CaptureDevice) {
  const flags = [
    device.is_suggested ? "sugerida" : "",
    device.is_virtual ? "virtual" : "",
    device.is_loopback ? "loopback" : "",
  ].filter(Boolean);

  return `${device.description || device.name}${flags.length ? ` (${flags.join(", ")})` : ""}`;
}

function directionLabel(direction: FlowDirection) {
  if (direction === "Outbound") {
    return "Saída";
  }
  if (direction === "Inbound") {
    return "Entrada";
  }
  return "Desconhecida";
}

function flowState(flow: NetworkFlow, running: boolean) {
  if (!running) {
    return "encerrado/parado";
  }
  return flow.active_now ? "ativo agora" : "inativo";
}

function flowKey(flow: NetworkFlow) {
  return [
    flow.protocol,
    flow.source_ip,
    flow.source_port ?? "",
    flow.destination_ip,
    flow.destination_port ?? "",
  ].join("|");
}

function formatTimestamp(value?: string | null) {
  if (!value) {
    return "-";
  }

  const seconds = Number(value);
  if (!Number.isFinite(seconds)) {
    return value;
  }

  return new Date(seconds * 1000).toLocaleTimeString("pt-BR");
}

function formatDuration(value?: number | null) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "-";
  }

  return `${value.toFixed(1)}s`;
}

export default App;
