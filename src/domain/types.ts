export type City =
  | "Martlock"
  | "Bridgewatch"
  | "Fort Sterling"
  | "Lymhurst"
  | "Thetford";

export type Quality = "normal" | "good" | "outstanding" | "excellent" | "masterpiece";

export interface Item {
  id: string;
  name: string;
  tier: number;
  enchantment: number;
  category: string;
}

export interface RecipeMaterial {
  itemId: string;
  quantity: number;
}

export interface Recipe {
  id: string;
  outputItemId: string;
  outputQuantity: number;
  stationType: string;
  focusCostPerCraft: number;
  materials: RecipeMaterial[];
}

export interface MarketPrice {
  itemId: string;
  city: City;
  quality: Quality;
  sellPrice: number;
  buyPrice: number;
  sellQuantity: number;
  buyQuantity: number;
  source: "manual" | "mock";
  updatedAt: string;
}

export interface InventoryStack {
  itemId: string;
  quantity: number;
  quality: Quality;
}

export interface CraftingSettings {
  city: City;
  premiumActive: boolean;
  focusAvailable: number;
  marketFeeRate: number;
  stationTaxRate: number;
  baseResourceReturnRate: number;
  focusResourceReturnRate: number;
  cityBonusRate: number;
  dailyBonusRate: number;
}

export interface CraftingMaterialResult {
  itemId: string;
  requiredBeforeReturn: number;
  expectedReturned: number;
  expectedConsumed: number;
  ownedQuantity: number;
  missingQuantity: number;
  unitBuyPrice: number;
  unitSellValue: number;
  cashCost: number;
  opportunityCost: number;
}

export interface CraftingResult {
  recipeId: string;
  outputItemId: string;
  city: City;
  craftCount: number;
  outputQuantity: number;
  useFocus: boolean;
  focusCost: number;
  resourceReturnRate: number;
  materials: CraftingMaterialResult[];
  grossRevenue: number;
  marketFee: number;
  netRevenue: number;
  stationFee: number;
  totalCashMaterialCost: number;
  totalOpportunityCost: number;
  cashProfit: number;
  economicProfit: number;
  roi: number;
  explanation: string[];
}

export interface FocusComparison {
  withoutFocus: CraftingResult;
  withFocus: CraftingResult;
  additionalEconomicProfit: number;
  additionalProfitPerFocusPoint: number;
}
