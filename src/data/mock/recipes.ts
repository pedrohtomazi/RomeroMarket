import type { Recipe } from "../../domain/types";

export const recipes: Recipe[] = [
  {
    id: "craft_bag_t4",
    outputItemId: "bag_t4",
    outputQuantity: 1,
    stationType: "toolmaker",
    focusCostPerCraft: 140,
    materials: [
      { itemId: "leather_t4", quantity: 8 },
      { itemId: "cloth_t4", quantity: 8 },
    ],
  },
  {
    id: "craft_cleric_robe_t4",
    outputItemId: "cleric_robe_t4",
    outputQuantity: 1,
    stationType: "mage-tower",
    focusCostPerCraft: 160,
    materials: [{ itemId: "cloth_t4", quantity: 16 }],
  },
  {
    id: "craft_broadsword_t4",
    outputItemId: "broadsword_t4",
    outputQuantity: 1,
    stationType: "warrior-forge",
    focusCostPerCraft: 180,
    materials: [
      { itemId: "metalbar_t4", quantity: 12 },
      { itemId: "plank_t4", quantity: 4 },
    ],
  },
];
