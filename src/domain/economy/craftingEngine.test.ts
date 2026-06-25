import { describe, expect, it } from "vitest";
import { recipes } from "../../data/mock/recipes";
import { analyzeCrafting, compareCraftingFocus } from "./craftingEngine";
import type { CraftingSettings } from "../types";

const settings: CraftingSettings = {
  city: "Martlock",
  premiumActive: true,
  focusAvailable: 12_000,
  marketFeeRate: 0.065,
  stationTaxRate: 0.085,
  baseResourceReturnRate: 0.152,
  focusResourceReturnRate: 0.435,
  cityBonusRate: 0.1,
  dailyBonusRate: 0.05,
};

describe("craftingEngine", () => {
  it("separates cash profit from economic profit", () => {
    const result = analyzeCrafting({
      recipe: recipes[0],
      craftCount: 10,
      settings,
      useFocus: false,
    });

    expect(result.totalCashMaterialCost).toBeLessThan(result.totalOpportunityCost);
    expect(result.cashProfit).not.toBe(result.economicProfit);
  });

  it("improves economic profit when focus increases resource return", () => {
    const comparison = compareCraftingFocus({
      recipe: recipes[0],
      craftCount: 10,
      settings,
    });

    expect(comparison.withFocus.resourceReturnRate).toBeGreaterThan(
      comparison.withoutFocus.resourceReturnRate,
    );
    expect(comparison.additionalProfitPerFocusPoint).toBeGreaterThan(0);
  });

  it("throws a clear error for invalid quantities", () => {
    expect(() =>
      analyzeCrafting({
        recipe: recipes[0],
        craftCount: 0,
        settings,
        useFocus: false,
      }),
    ).toThrow("quantidade");
  });
});
