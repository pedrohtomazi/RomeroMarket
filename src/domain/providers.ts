import { inventoryStacks } from "../data/mock/inventory";
import { prices } from "../data/mock/prices";
import type { City, InventoryStack, MarketPrice, Quality } from "./types";

export interface MarketPriceProvider {
  getPrice(itemId: string, city: City, quality?: Quality): MarketPrice | undefined;
  listPrices(): MarketPrice[];
}

export interface InventoryProvider {
  listInventory(): InventoryStack[];
}

export class MockMarketPriceProvider implements MarketPriceProvider {
  getPrice(itemId: string, city: City, quality: Quality = "normal") {
    return prices.find(
      (price) =>
        price.itemId === itemId && price.city === city && price.quality === quality,
    );
  }

  listPrices() {
    return prices;
  }
}

export class MockInventoryProvider implements InventoryProvider {
  listInventory() {
    return inventoryStacks;
  }
}
