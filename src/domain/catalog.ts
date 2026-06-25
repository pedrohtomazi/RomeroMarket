import { items } from "../data/mock/items";
import type { Item } from "./types";

export const itemCatalog: Item[] = items;

export const itemsById: Record<string, Item> = Object.fromEntries(
  itemCatalog.map((item) => [item.id, item]),
);
