// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  output: 'harbor.svg',
  viewBox: [-136, -56, 272, 200],
  origin: [0, 0],
  layers: [
    { id: 'ground', name: 'Ground' },
    { id: 'grass', name: 'Grass', parent: 'ground' },
    { id: 'paths', name: 'Paths', parent: 'ground' },
    { id: 'beach', name: 'Beach', parent: 'ground' },
    { id: 'water', name: 'Water', parent: 'ground' },
    { id: 'village', name: 'Village' },
    { id: 'houses', name: 'Houses', parent: 'village' },
    { id: 'red-house', name: 'Red house', parent: 'houses' },
    { id: 'wood-house', name: 'Wood house', parent: 'houses' },
    { id: 'town-hall', name: 'Town hall', parent: 'houses' },
    { id: 'garden', name: 'Garden', parent: 'village' },
    { id: 'harbor', name: 'Harbor' },
    { id: 'dock', name: 'Dock', parent: 'harbor' },
    { id: 'boat', name: 'Boat', parent: 'harbor' },
    { id: 'lamps', name: 'Lamps', parent: 'harbor' },
    { id: 'trees', name: 'Trees' },
  ],
  items: [
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '0', 'data-x': '0', 'data-y': '0' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [0, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '0', 'data-y': '0' },
      items: [
        { object: 'tree', layer: 'trees', at: { tile: [0, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '0', 'data-x': '16', 'data-y': '8' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [1, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '1', 'data-x': '-16', 'data-y': '8' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [0, 1, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '0', 'data-x': '32', 'data-y': '16' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [2, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '1', 'data-x': '0', 'data-y': '16' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [1, 1, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '2', 'data-x': '-32', 'data-y': '16' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [0, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-32', 'data-y': '16' },
      items: [
        { object: 'tree', layer: 'garden', at: { tile: [0, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '0', 'data-y': '0' },
      items: [
        { object: 'walls', layer: 'red-house', at: { tile: [1, 1, 1] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '0', 'data-y': '-16' },
      items: [
        { object: 'roof-red', layer: 'red-house', at: { tile: [1, 1, 2] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '0', 'data-x': '48', 'data-y': '24' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [3, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '1', 'data-x': '16', 'data-y': '24' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [2, 1, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '2', 'data-x': '-16', 'data-y': '24' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [1, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '3', 'data-x': '-48', 'data-y': '24' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [0, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '48', 'data-y': '24' },
      items: [
        { object: 'pine', layer: 'trees', at: { tile: [3, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '0', 'data-x': '64', 'data-y': '32' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [4, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '1', 'data-x': '32', 'data-y': '32' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [3, 1, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '2', 'data-x': '0', 'data-y': '32' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [2, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '3', 'data-x': '-32', 'data-y': '32' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [1, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '4', 'data-x': '-64', 'data-y': '32' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [0, 4, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-32', 'data-y': '32' },
      items: [
        { object: 'flowers', layer: 'garden', at: { tile: [1, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-64', 'data-y': '32' },
      items: [
        { object: 'tree', layer: 'trees', at: { tile: [0, 4, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '0', 'data-x': '80', 'data-y': '40' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [5, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '1', 'data-x': '48', 'data-y': '40' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [4, 1, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '2', 'data-x': '16', 'data-y': '40' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [3, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '3', 'data-x': '-16', 'data-y': '40' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [2, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '4', 'data-x': '-48', 'data-y': '40' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [1, 4, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '5', 'data-x': '-80', 'data-y': '40' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [0, 5, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '80', 'data-y': '40' },
      items: [
        { object: 'tree', layer: 'trees', at: { tile: [5, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '48', 'data-y': '24' },
      items: [
        { object: 'walls', layer: 'wood-house', at: { tile: [4, 1, 1] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '48', 'data-y': '8' },
      items: [
        { object: 'roof-wood', layer: 'wood-house', at: { tile: [4, 1, 2] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '6', 'data-row': '0', 'data-x': '96', 'data-y': '48' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [6, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '1', 'data-x': '64', 'data-y': '48' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [5, 1, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '2', 'data-x': '32', 'data-y': '48' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [4, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '3', 'data-x': '0', 'data-y': '48' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [3, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '4', 'data-x': '-32', 'data-y': '48' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [2, 4, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '5', 'data-x': '-64', 'data-y': '48' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [1, 5, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '6', 'data-x': '-96', 'data-y': '48' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [0, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '0', 'data-y': '48' },
      items: [
        { object: 'flowers', layer: 'garden', at: { tile: [3, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-96', 'data-y': '48' },
      items: [
        { object: 'pine', layer: 'trees', at: { tile: [0, 6, 0] } },
      ],
    },
    { object: 'water', layer: 'water', at: { tile: [7, 0, -1] } },
    { object: 'boat', id: 'boat', layer: 'boat', at: { tile: [7, 0, -1] } },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '6', 'data-row': '1', 'data-x': '80', 'data-y': '56' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [6, 1, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '2', 'data-x': '48', 'data-y': '56' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [5, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '3', 'data-x': '16', 'data-y': '56' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [4, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '4', 'data-x': '-16', 'data-y': '56' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [3, 4, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '5', 'data-x': '-48', 'data-y': '56' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [2, 5, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '6', 'data-x': '-80', 'data-y': '56' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [1, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '7', 'data-x': '-112', 'data-y': '56' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [0, 7, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-80', 'data-y': '56' },
      items: [
        { object: 'tree', layer: 'trees', at: { tile: [1, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '48', 'data-y': '56' },
      items: [
        { object: 'tree', layer: 'trees', at: { tile: [5, 2, 0] } },
      ],
    },
    { object: 'rock', at: { tile: [0, 7, 0] } },
    { object: 'water', layer: 'water', at: { tile: [7, 1, -1] } },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '6', 'data-row': '2', 'data-x': '64', 'data-y': '64' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [6, 2, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '3', 'data-x': '32', 'data-y': '64' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [5, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '4', 'data-x': '0', 'data-y': '64' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [4, 4, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '5', 'data-x': '-32', 'data-y': '64' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [3, 5, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '6', 'data-x': '-64', 'data-y': '64' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [2, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '7', 'data-x': '-96', 'data-y': '64' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [1, 7, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '32', 'data-y': '64' },
      items: [
        { object: 'lamp', layer: 'lamps', at: { tile: [5, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-48', 'data-y': '40' },
      items: [
        { object: 'hall-walls', layer: 'town-hall', at: { tile: [2, 5, 1] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-48', 'data-y': '24' },
      items: [
        { object: 'hall-roof', layer: 'town-hall', at: { tile: [2, 5, 2] } },
      ],
    },
    { object: 'water', layer: 'water', at: { tile: [7, 2, -1] } },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '6', 'data-row': '3', 'data-x': '48', 'data-y': '72' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [6, 3, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '4', 'data-x': '16', 'data-y': '72' },
      items: [
        { object: 'path', layer: 'paths', at: { tile: [5, 4, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '5', 'data-x': '-16', 'data-y': '72' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [4, 5, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '6', 'data-x': '-48', 'data-y': '72' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [3, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '7', 'data-x': '-80', 'data-y': '72' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [2, 7, 0] } },
      ],
    },
    { object: 'barrel', layer: 'dock', at: { tile: [6, 3, 0] } },
    { object: 'water', layer: 'water', at: { tile: [7, 3, -1] } },
    { object: 'dock', layer: 'dock', at: { tile: [6, 4, 0] } },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '5', 'data-x': '0', 'data-y': '80' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [5, 5, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '6', 'data-x': '-32', 'data-y': '80' },
      items: [
        { object: 'grass', layer: 'grass', at: { tile: [4, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '3', 'data-row': '7', 'data-x': '-64', 'data-y': '80' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [3, 7, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-32', 'data-y': '80' },
      items: [
        { object: 'flowers', layer: 'garden', at: { tile: [4, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '0', 'data-y': '80' },
      items: [
        { object: 'lamp', layer: 'lamps', at: { tile: [5, 5, 0] } },
      ],
    },
    { object: 'dock', layer: 'dock', at: { tile: [7, 4, 0] } },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '6', 'data-row': '5', 'data-x': '16', 'data-y': '88' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [6, 5, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '6', 'data-x': '-16', 'data-y': '88' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [5, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '4', 'data-row': '7', 'data-x': '-48', 'data-y': '88' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [4, 7, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '-16', 'data-y': '88' },
      items: [
        { object: 'pine', layer: 'trees', at: { tile: [5, 6, 0] } },
      ],
    },
    { object: 'barrel', layer: 'dock', at: { tile: [7, 4, 0] } },
    { object: 'water', layer: 'water', at: { tile: [7, 5, -1] } },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '6', 'data-row': '6', 'data-x': '0', 'data-y': '96' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [6, 6, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '5', 'data-row': '7', 'data-x': '-32', 'data-y': '96' },
      items: [
        { object: 'sand', layer: 'beach', at: { tile: [5, 7, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '0', 'data-y': '96' },
      items: [
        { object: 'rock', at: { tile: [6, 6, 0] } },
      ],
    },
    { object: 'water', layer: 'water', at: { tile: [7, 6, -1] } },
    { object: 'water', layer: 'water', at: { tile: [6, 7, -1] } },
    { object: 'water', layer: 'water', at: { tile: [7, 7, -1] } },
  ],
  motion: {
    duration: 8000,
    tracks: [
      {
        item: 'boat',
        keys: [
          { t: 0 },
          { t: 3500, move: [-48, 24] },
          { t: 4500, move: [-48, 24] },
        ],
      },
    ],
  },
};
