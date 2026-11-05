<?php

namespace App\Support;

/** 500 made-up products, the same every time: what both sides of /compare search. */
class Products
{
    public static function all(): array
    {
        mt_srand(7);
        $adjectives = ['Classic', 'Compact', 'Deluxe', 'Eco', 'Heavy', 'Light', 'Mini', 'Pro', 'Smart', 'Ultra'];
        $things = ['Backpack', 'Blender', 'Desk Lamp', 'Headphones', 'Kettle', 'Keyboard', 'Monitor', 'Mug', 'Notebook', 'Speaker', 'Tent', 'Water Bottle'];
        $categories = ['Home', 'Office', 'Outdoors', 'Kitchen', 'Audio'];

        $products = [];
        for ($id = 1; $id <= 500; $id++) {
            $products[] = [
                'id' => $id,
                'name' => $adjectives[mt_rand(0, 9)] . ' ' . $things[mt_rand(0, 11)] . ' ' . mt_rand(100, 999),
                'category' => $categories[mt_rand(0, 4)],
                'price' => mt_rand(500, 49900),   // cents
            ];
        }

        return $products;
    }

    /** The ones whose name or category has `$query` in it. */
    public static function search(array $products, string $query): array
    {
        $query = strtolower(trim($query));
        if ($query === '') {
            return $products;
        }

        return array_values(array_filter($products, fn($p) => str_contains(strtolower($p['name'] . ' ' . $p['category']), $query)));
    }
}
