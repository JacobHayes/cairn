# Structure checks beside every screenshot

Every screenshot state is now checked from Chrome's own layout snapshot, and `mise run screenshots` saves two small files beside each picture for the screen critique to read: what the rules passed and what came near them, and the screen's roles and names. Both are build output, not committed.

## What a `.checks.json` holds

`journey-list-phone-light.checks.json` (trimmed):

```json
{
  "passed": ["elements outside their boxes or text cut without a sign", "a key shown where a name belongs",
             "a native tooltip", "controls under the touch target", "text under the type floor"],
  "near": {
    "ellipsised": [],
    "smallestHitAreas": [{ "name": "button[journey-menu] \"Journey actions\"", "width": 44, "height": 44 }],
    "fontSizes": { "12": 30, "13": 3, "14": 37, "15": 52, "26.571": 1 }
  }
}
```

## What the trimmed accessibility snapshot looks like

`journey-list-desktop-light.aria.yml` (8 KB for the whole list screen; link targets, text runs and values removed):

```yaml
- navigation "Screens":
  - link "Home"
  - link "Journeys"
- main:
  - region "Evaluate the analytics vendor":
    - heading "Evaluate the analytics vendor" [level=1]
    - 'button "View: List"'
  - region "List":
    - toolbar "View controls":
      - button "Frontier"
      - search:
        - searchbox "Search"
```

## What the rules caught

| Rule | Before | After |
| --- | --- | --- |
| Text under 12px (status chips, mono labels, key caps, axis text) | chips at 11 and 11.5px, labels at 10.5 and 11px | one 12px label size, `--type-label-size` |

## Time

| | Before | After |
| --- | --- | --- |
| Screenshot walk, 142 states | 36 s (phone 33 s) | 21 s |

The phone walk was dominated by probing every control by points; boxes and paint order decide the controls that are plainly large enough, and only the rest are probed.

## Known limits

- A hit area from boxes misses rounded corners, clip-paths and rotation; the last two are probed. The decision record lists each limit.
