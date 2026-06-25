import { inventoryStacks } from "../../data/mock/inventory";
import { prices } from "../../data/mock/prices";
import type {
  CraftingResult,
  CraftingSettings,
  FocusComparison,
  InventoryStack,
  MarketPrice,
  Recipe,
} from "../types";

interface AnalyzeCraftingInput {
  recipe: Recipe;
  craftCount: number;
  settings: CraftingSettings;
  useFocus: boolean;
  inventory?: InventoryStack[];
  marketPrices?: MarketPrice[];
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), max);

export function analyzeCrafting({
  recipe,
  craftCount,
  settings,
  useFocus,
  inventory = inventoryStacks,
  marketPrices = prices,
}: AnalyzeCraftingInput): CraftingResult {
  if (!Number.isFinite(craftCount) || craftCount <= 0) {
    throw new Error("A quantidade de crafts deve ser maior que zero.");
  }

  const resourceReturnRate = clamp(
    settings.baseResourceReturnRate +
      settings.cityBonusRate +
      settings.dailyBonusRate +
      (useFocus ? settings.focusResourceReturnRate : 0),
    0,
    0.95,
  );

  const focusCost = useFocus ? recipe.focusCostPerCraft * craftCount : 0;
  const outputQuantity = recipe.outputQuantity * craftCount;
  const outputPrice = requirePrice(
    marketPrices,
    recipe.outputItemId,
    settings.city,
  );

  const materials = recipe.materials.map((material) => {
    const requiredBeforeReturn = material.quantity * craftCount;
    const expectedReturned = round(requiredBeforeReturn * resourceReturnRate);
    const expectedConsumed = round(requiredBeforeReturn - expectedReturned);
    const ownedQuantity = inventory
      .filter((stack) => stack.itemId === material.itemId)
      .reduce((sum, stack) => sum + stack.quantity, 0);
    const missingQuantity = Math.max(0, requiredBeforeReturn - ownedQuantity);
    const price = requirePrice(marketPrices, material.itemId, settings.city);

    return {
      itemId: material.itemId,
      requiredBeforeReturn,
      expectedReturned,
      expectedConsumed,
      ownedQuantity,
      missingQuantity,
      unitBuyPrice: price.sellPrice,
      unitSellValue: price.buyPrice,
      cashCost: missingQuantity * price.sellPrice,
      opportunityCost: expectedConsumed * price.buyPrice,
    };
  });

  const grossRevenue = outputQuantity * outputPrice.buyPrice;
  const marketFee = grossRevenue * settings.marketFeeRate;
  const netRevenue = grossRevenue - marketFee;
  const grossMaterialMarketValue = materials.reduce(
    (sum, line) => sum + line.requiredBeforeReturn * line.unitBuyPrice,
    0,
  );
  const stationFee = grossMaterialMarketValue * settings.stationTaxRate;
  const totalCashMaterialCost = materials.reduce(
    (sum, line) => sum + line.cashCost,
    0,
  );
  const totalOpportunityCost = materials.reduce(
    (sum, line) => sum + line.opportunityCost,
    0,
  );
  const cashProfit = netRevenue - totalCashMaterialCost - stationFee;
  const economicProfit = netRevenue - totalOpportunityCost - stationFee;
  const invested = totalOpportunityCost + stationFee;
  const roi = invested > 0 ? economicProfit / invested : 0;

  return {
    recipeId: recipe.id,
    outputItemId: recipe.outputItemId,
    city: settings.city,
    craftCount,
    outputQuantity,
    useFocus,
    focusCost,
    resourceReturnRate,
    materials,
    grossRevenue,
    marketFee,
    netRevenue,
    stationFee,
    totalCashMaterialCost,
    totalOpportunityCost,
    cashProfit,
    economicProfit,
    roi,
    explanation: [
      "Precos e inventario vêm de mocks locais.",
      "Lucro em caixa desconta apenas materiais faltantes comprados.",
      "Lucro economico desconta o valor de venda dos materiais consumidos.",
      "Taxa da estacao usa uma formula simplificada baseada no valor bruto dos materiais.",
    ],
  };
}

export function compareCraftingFocus(input: {
  recipe: Recipe;
  craftCount: number;
  settings: CraftingSettings;
}): FocusComparison {
  const withoutFocus = analyzeCrafting({ ...input, useFocus: false });
  const withFocus = analyzeCrafting({ ...input, useFocus: true });
  const additionalEconomicProfit =
    withFocus.economicProfit - withoutFocus.economicProfit;
  const additionalProfitPerFocusPoint =
    withFocus.focusCost > 0 ? additionalEconomicProfit / withFocus.focusCost : 0;

  return {
    withoutFocus,
    withFocus,
    additionalEconomicProfit,
    additionalProfitPerFocusPoint,
  };
}

function requirePrice(
  marketPrices: MarketPrice[],
  itemId: string,
  city: string,
): MarketPrice {
  const price = marketPrices.find(
    (entry) =>
      entry.itemId === itemId && entry.city === city && entry.quality === "normal",
  );

  if (!price) {
    throw new Error(`Preco ausente para ${itemId} em ${city}.`);
  }

  return price;
}

function round(value: number) {
  return Math.round(value * 100) / 100;
}
