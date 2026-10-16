#!/usr/bin/env python3
"""Import Bryan James's original polygons and CSS transitions as numeric Rust data.

Development-only dependencies: tinycss2, cssselect2. The rendered video has no
browser, CSS parser, network dependency or SVG path strings.

Run: python scripts/import_original.py --cache /tmp/in-pieces-source
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import urllib.request
import xml.etree.ElementTree as ET

import cssselect2
import tinycss2

COMMIT = 'd81b692dc3aca4b3e4e7f5e9f8d9635094faea05'
ROOT = Path(__file__).resolve().parents[1]
ANIMALS = 'crow vaquita tamarin frog owl turtle oryx iguana seahorse armadillo sloth kakapo echidna penguin damselfly bear parrotfish camel'.split()
CLASSES = ['', 'animal-animations-on'] + [f'animal-animations-on state-{s}' for s in ['two', 'three', 'four']]
CLASSES += [s + ' two-state-two' for s in CLASSES[1:]]
EASE = (0.25, 0.1, 0.25, 1.0)


def split_list(text):
    return re.split(r',\s*(?![^()]*\))', text)


def timing_list(text):
    result = []
    for part in split_list(text):
        bezier = re.search(r'cubic-bezier\(([^)]+)\)', part)
        curve = tuple(map(float, bezier[1].split(','))) if bezier else EASE
        rest = re.sub(r'cubic-bezier\([^)]+\)', '', part)
        durations = []
        prop = 'all'
        for word in rest.split():
            if re.fullmatch(r'-?[\d.]+m?s', word):
                durations.append(float(word[:-2]) / 1000 if word.endswith('ms') else float(word[:-1]))
            elif word == 'linear':
                curve = (0., 0., 1., 1.)
            elif word in ['ease-in', 'ease-out', 'ease-in-out', 'ease']:
                curve = {'ease': EASE, 'ease-in': (.42, 0., 1., 1.), 'ease-out': (0., 0., .58, 1.), 'ease-in-out': (.42, 0., .58, 1.)}[word]
            else:
                prop = word.removeprefix('-webkit-')
        result.append([prop, durations[0] if durations else 0., durations[1] if len(durations) > 1 else 0., curve])
    return result


def declarations(rule):
    # Expand transition shorthands before cascading. A later shorthand resets
    # earlier longhands; treating these as independent strings gets blinks wrong.
    result = {}
    for d in tinycss2.parse_declaration_list(rule.content, skip_comments=True, skip_whitespace=True):
        if d.type != 'declaration' or d.lower_name.startswith(('-moz-', '-ms-', '-o-')):
            continue
        key = d.lower_name.removeprefix('-webkit-')
        value = tinycss2.serialize(d.value).strip()
        if key == 'transition':
            try:
                transitions = timing_list(value)
            except ValueError:
                continue
            for index, name in enumerate(['property', 'duration', 'delay', 'timing-function']):
                result[f'transition-{name}'] = [t[index] for t in transitions]
        elif key in ['transition-duration', 'transition-delay']:
            if all(re.fullmatch(r'-?[\d.]+m?s', v) for v in split_list(value)):
                result[key] = [float(v[:-2]) / 1000 if v.endswith('ms') else float(v[:-1]) for v in split_list(value)]
        else:
            result[key] = value
    return result


def colour(text):
    if text.startswith('#'):
        h = text[1:]
        if len(h) == 3:
            h = ''.join(c * 2 for c in h)
        return (*tuple(int(h[i:i + 2], 16) for i in [0, 2, 4]), 255)
    if text == 'white':
        return (255, 255, 255, 255)
    if text.startswith('rgba('):
        r, g, b, a = map(float, text[5:-1].split(','))
        return (int(r), int(g), int(b), round(a * 255))
    raise ValueError(text)


def timing(style, prop):
    names = style.get('transition-property', ['all'])
    matching = [i for i, p in enumerate(names) if p in ['all', prop]]
    if not matching:
        return (0., 0., *EASE)
    i = matching[-1]
    def get(name, default):
        values = style.get(f'transition-{name}', [default])
        return values[i % len(values)]
    return (get('duration', 0.), get('delay', 0.), *get('timing-function', EASE))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, default=Path('/tmp/in-pieces-source'))
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)
    sources = {}
    for file in ['css/main.css', 'js/main.js', 'js/vars.js']:
        target = args.cache / Path(file).name
        url = f'https://raw.githubusercontent.com/bryanjamesinteractive/in-pieces/{COMMIT}/{file}'
        if not target.exists():
            target.write_bytes(urllib.request.urlopen(url).read())
        sources[file] = {'url': url, 'sha256': hashlib.sha256(target.read_bytes()).hexdigest()}
    matcher = cssselect2.Matcher()
    for rule in tinycss2.parse_stylesheet((args.cache / 'main.css').read_text(), skip_comments=True, skip_whitespace=True):
        # Only unconditional rules define the polygons and their motions. Media
        # rules concern website UI and responsive placement, not their geometry.
        if rule.type != 'qualified-rule':
            continue
        try:
            selectors = cssselect2.compile_selector_list(rule.prelude)
        except cssselect2.SelectorError:
            continue
        values = declarations(rule)
        for selector in selectors:
            matcher.add_selector(selector, values)

    def resolve(element):
        values = {}
        for specificity, order, pseudo, payload in matcher.match(element):
            if pseudo is None:
                values.update(payload)
        return values

    shapes, timings, cells = [], [], []
    def intern(array, value):
        if value not in array:
            array.append(value)
        return array.index(value)

    imported = []
    for animal in ANIMALS:
        species = {'id': animal, 'states': [], 'movers': [], 'bubbles': []}
        for state in CLASSES:
            html = ET.Element('html', {'class': 'webkitbrowser chromebrowser'})
            body = ET.SubElement(html, 'body', {'class': state, 'id': 'allanimals'})
            # Preserve #animalchanger's actual :nth-child position in index.html.
            ET.SubElement(body, 'div', {'id': 'fb-root'})
            ET.SubElement(body, 'script')
            changer = ET.SubElement(body, 'div', {'id': 'animalchanger', 'class': animal})
            parent = changer
            for name in ['level-one', 'level-two', 'animal-mover', 'wrap left-to-right']:
                parent = ET.SubElement(parent, 'div', {'class': name})
            for i in range(33):
                wrapper = ET.SubElement(parent, 'div', {'class': 'shard-wrap' + (' peripheral' if i >= 30 else '')})
                ET.SubElement(wrapper, 'div', {'class': 'shard'})
            state_cells = []
            for el in cssselect2.ElementWrapper.from_html_root(html).iter_subtree():
                style = resolve(el)
                if el.id == 'animalchanger':
                    species['background'] = colour(style['background-color'])
                if 'animal-mover' in el.classes:
                    offset = [0., 0.]
                    transform = style.get('transform', 'none')
                    for direction, amount, unit in re.findall(r'translate([XY])\((-?[\d.]+)(%|px)\)', transform):
                        # Percent of the original full viewport. Our SVG keeps
                        # the original 100:70 illustration coordinate system.
                        axis = 0 if direction == 'X' else 1
                        offset[axis] = float(amount) / .99 if unit == '%' else float(amount) / (19.008 if axis == 0 else 13.3056)
                    species['movers'].append((offset, intern(timings, timing(style, 'transform'))))
                if 'shard' not in el.classes:
                    continue
                coords = tuple(map(float, re.findall(r'(-?[\d.]+)%', style['clip-path'])))
                if len(coords) != 6:
                    raise ValueError((animal, state, coords))
                shape = intern(shapes, (coords, colour(style['background-color'])))
                z = int(resolve(el.parent).get('z-index', '2'))
                cell = (shape, intern(timings, timing(style, 'clip-path')), intern(timings, timing(style, 'background-color')), z)
                state_cells.append(intern(cells, cell))
                if not state:
                    species['bubbles'].append(float(style.get('animation-delay', '0s')[:-1]) if style.get('animation', '').startswith('bubbles') else -1.)
            assert len(state_cells) == 33
            species['states'].append(state_cells)
        imported.append(species)

    def f(value):
        return f'{value:.6f}'.rstrip('0').rstrip('.') + ('.' if float(value).is_integer() else '')
    output = [
        '//! Generated from the original In Pieces CSS. Do not edit by hand.',
        f'//! Source commit: {COMMIT}. Regenerate with `scripts/import_original.py`.',
        '#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::unreadable_literal)]',
        'use fframes::Color;',
        'use crate::geometry::{Path, Piece};',
        'use crate::choreography::{Cell, CssTiming, Mover, SourceSpecies};',
        f'pub static SHAPES: [Piece; {len(shapes)}] = [',
    ]
    for coords, rgb in shapes:
        points = ', '.join(f'({f(coords[i])}, {f(coords[i+1])})' for i in [0, 2, 4])
        output.append(f'Piece {{ path: Path {{ points: [{points}] }}, color: Color::rgba{rgb} }},')
    output += ['];', f'pub static TIMINGS: [CssTiming; {len(timings)}] = [']
    for duration, delay, *curve in timings:
        output.append(f'CssTiming {{ duration: {f(duration)}, delay: {f(delay)}, curve: [{", ".join(map(f, curve))}] }},')
    output += ['];', f'pub static CELLS: [Cell; {len(cells)}] = [']
    for shape, path, color, z in cells:
        output.append(f'Cell {{ shape: {shape}, path_timing: {path}, color_timing: {color}, z: {z} }},')
    output += ['];', 'pub static SPECIES: [SourceSpecies; 18] = [']
    for species in imported:
        output.append(f'SourceSpecies {{ id: "{species["id"]}", background: Color::rgba{species["background"]}, states: [')
        output += [f'{state},' for state in species['states']]
        output.append('], movers: [')
        for (x, y), time in species['movers']:
            output.append(f'Mover {{ offset: ({f(x)}, {f(y)}), timing: {time} }},')
        output.append('], bubbles: [' + ', '.join(map(f, species['bubbles'])) + '] },')
    output.append('];')
    (ROOT / 'src' / 'source_data.rs').write_text('\n'.join(output) + '\n')
    manifest = {'commit': COMMIT, 'sources': sources, 'order': ANIMALS, 'species': 18, 'persistent_pieces': 30, 'peripheral_pieces': 3, 'shapes': len(shapes), 'timings': len(timings), 'cells': len(cells), 'states': CLASSES, 'seconds_per_species': 10, 'total_seconds': 180}
    (ROOT / 'source-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps({k: manifest[k] for k in ['species', 'shapes', 'timings', 'cells', 'total_seconds']}))


if __name__ == '__main__':
    main()
