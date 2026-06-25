import {
  BarChart3,
  Boxes,
  Calculator,
  Home,
  Settings,
  Store,
} from "lucide-react";
import { useMemo, useState } from "react";
import "./App.css";
import { recipes } from "./data/mock/recipes";
import { inventoryStacks } from "./data/mock/inventory";
import { prices } from "./data/mock/prices";
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
  | "settings";

const navItems: Array<{ id: ViewId; label: string; icon: typeof Home }> = [
  { id: "home", label: "Inicio", icon: Home },
  { id: "crafting", label: "Crafting", icon: Calculator },
  { id: "opportunities", label: "Oportunidades", icon: BarChart3 },
  { id: "market", label: "Mercado", icon: Store },
  { id: "inventory", label: "Inventario", icon: Boxes },
  { id: "settings", label: "Configuracoes", icon: Settings },
];

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

const silver = new Intl.NumberFormat("pt-BR", {
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
              <Metric label="Itens no inventario" value={inventoryStacks.length.toString()} />
              <Metric label="Precos mock" value={prices.length.toString()} />
              <Metric
                label="Melhor lucro economico"
                value={`${silver.format(opportunities[0]?.economicProfit ?? 0)} prata`}
              />
            </div>
          </section>
        )}

        {activeView === "crafting" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">Motor economico</p>
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
                label="Lucro economico"
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
                      <th>Necessario</th>
                      <th>Possuido</th>
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
              <Panel title="Calculo">
                <dl className="details">
                  <div>
                    <dt>Receita liquida</dt>
                    <dd>{silver.format(craftingResult.netRevenue)}</dd>
                  </div>
                  <div>
                    <dt>Taxa da estacao</dt>
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
                <p className="eyebrow">Analise</p>
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
                    <th>Lucro economico</th>
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
                <p className="eyebrow">Precos</p>
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
                      <td>{price.sellQuantity}/{price.buyQuantity}</td>
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
                <h1>Inventario</h1>
              </div>
              <StatusPill label="Simulado" />
            </header>
            <Panel title="Itens possuidos">
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

        {activeView === "settings" && (
          <section className="view">
            <header className="viewHeader">
              <div>
                <p className="eyebrow">Perfil</p>
                <h1>Configuracoes</h1>
              </div>
              <StatusPill label={settings.premiumActive ? "Premium ativo" : "Sem premium"} />
            </header>
            <div className="metricGrid">
              <Metric label="Foco disponivel" value={silver.format(settings.focusAvailable)} />
              <Metric label="Taxa de mercado" value={`${settings.marketFeeRate * 100}%`} />
              <Metric label="Taxa de estacao" value={`${settings.stationTaxRate * 100}%`} />
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

export default App;
