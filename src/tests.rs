use super::*;

mod translation_tests {
    use super::{
        decode_table_text, encode_cp1252, find_padded_render_translation, load_render_translations,
        load_translations, render_overrides, translate_bare_report_date,
        translate_chronicle_header, translate_city_signature, translate_day_count,
        translate_goods_supply_town, translate_lord_mayor, translate_manager_stage,
        translate_pirate_captain_intro, translate_pirate_capture, translate_report_date,
        translate_tavern_sailors, translate_town_information, RenderTranslations,
    };
    use std::{collections::HashMap, ffi::CString};

    #[test]
    fn table_escapes_preserve_carriage_returns() {
        let decoded = decode_table_text(r"line\nreturn\x0D");
        assert_eq!(decoded, "line\nreturn\r");
        assert_eq!(encode_cp1252(&decoded, "test").unwrap(), b"line\nreturn\r");
        assert_eq!(decode_table_text(r"\r"), r"\r");
    }

    #[test]
    fn translates_observed_forms_and_keeps_values() {
        assert_eq!(
            translate_town_information(b"Population: 1700")
                .unwrap()
                .as_bytes(),
            b"Einwohnerzahl: 1700"
        );
        assert_eq!(
            translate_town_information(b"70.000 inhabitants")
                .unwrap()
                .as_bytes(),
            b"70.000 Einwohner"
        );
        assert!(translate_town_information(b"several inhabitants").is_none());
        assert_eq!(
            translate_town_information(b"Information about L\xfcbeck")
                .unwrap()
                .as_bytes(),
            b"Informationen \xfcber L\xfcbeck"
        );
    }

    #[test]
    fn leaves_unrecognized_text_alone() {
        assert!(translate_town_information(b"Population: unknown").is_none());
        assert!(translate_town_information(b"Information about ").is_none());
    }

    #[test]
    fn translates_lord_mayor_label_without_changing_the_name() {
        assert_eq!(
            translate_lord_mayor(b"Lord Mayor: the erfahrene Jakob Strohmberg")
                .unwrap()
                .as_bytes(),
            b"B\xfcrgermeister: der erfahrene Jakob Strohmberg"
        );
        assert_eq!(
            translate_lord_mayor(b"Lady Mayoress: the erfahrene Ada Hesse")
                .unwrap()
                .as_bytes(),
            b"B\xfcrgermeisterin: die erfahrene Ada Hesse"
        );
        assert!(translate_lord_mayor(b"Lord Mayor: the ").is_none());
        assert!(translate_lord_mayor(b"Lady Mayoress: the ").is_none());
    }

    #[test]
    fn translates_dynamic_tavern_sailor_count() {
        assert_eq!(
            translate_tavern_sailors(b"There are 28 sailors in the tavern.")
                .unwrap()
                .as_bytes(),
            b"In der Schenke sind 28 Seeleute."
        );
        assert!(translate_tavern_sailors(b"There are many sailors in the tavern.").is_none());
        assert!(translate_tavern_sailors(b"There are 28 sailors in the tavern!").is_none());
    }

    #[test]
    fn translates_pirate_capture_without_assuming_ship_gender() {
        assert_eq!(
            translate_pirate_capture(b"The pirates take the Kraier Sturmvogel")
                .unwrap()
                .as_bytes(),
            b"Das Schiff Kraier Sturmvogel wird von den Piraten gekapert."
        );
        assert_eq!(
            translate_pirate_capture(b"Die Piraten kapern Kogge Hanseat")
                .unwrap()
                .as_bytes(),
            b"Das Schiff Kogge Hanseat wird von den Piraten gekapert."
        );
        assert!(translate_pirate_capture(b"The pirates take ").is_none());
    }

    #[test]
    fn translates_dynamic_pirate_captain_count() {
        assert_eq!(
            translate_pirate_captain_intro(
                b"A fierce looking pirate captain with 14 henchmen offers"
            )
            .unwrap()
            .as_bytes(),
            b"Ein finster dreinblickender Piratenkapit\xe4n mit 14 Schergen bietet"
        );
        assert!(translate_pirate_captain_intro(
            b"A fierce looking pirate captain with many henchmen offers"
        )
        .is_none());
    }

    #[test]
    fn translates_dynamic_game_labels_without_losing_values() {
        assert_eq!(
            translate_manager_stage(b"Manager (Stage 3)")
                .unwrap()
                .as_bytes(),
            b"Verwalter (Stufe 3)"
        );
        assert!(translate_manager_stage(b"Manager (Stage III)").is_none());
    }

    #[test]
    fn localizes_report_dates_and_months_as_cp1252() {
        assert_eq!(
            translate_report_date(b"on 31. May 1345  ")
                .unwrap()
                .as_bytes(),
            b"am 31. Mai 1345  "
        );
        assert_eq!(
            translate_report_date(b"from 1. Mar 1345 to 2. Oct 1345")
                .unwrap()
                .as_bytes(),
            b"vom 1. M\xe4r 1345 bis zum 2. Okt 1345"
        );
        assert_eq!(
            translate_bare_report_date(b"1.Oct 1345")
                .unwrap()
                .as_bytes(),
            b"1.Okt 1345"
        );
        assert_eq!(
            translate_bare_report_date(b"15. Apr 1358")
                .unwrap()
                .as_bytes(),
            b"15. Apr 1358"
        );
        assert_eq!(
            translate_bare_report_date(b"15. May 1358")
                .unwrap()
                .as_bytes(),
            b"15. Mai 1358"
        );
        assert!(translate_bare_report_date(b"1.Jul").is_none());
    }

    #[test]
    fn localizes_chronicle_header_month_and_year() {
        assert_eq!(
            translate_chronicle_header(b"Chronik vom 27. Oct 1400")
                .unwrap()
                .as_bytes(),
            b"Chronik vom 27. Okt 1400"
        );
        assert_eq!(
            translate_chronicle_header(b"Chronik vom 15. Apr 1358")
                .unwrap()
                .as_bytes(),
            b"Chronik vom 15. Apr 1358"
        );
    }

    #[test]
    fn translates_dynamic_goods_supply_town_and_keeps_padding() {
        assert_eq!(
            translate_goods_supply_town(b"Goods supply in L\xfcbeck  ")
                .unwrap()
                .as_bytes(),
            b"Warenversorgung in L\xfcbeck  "
        );
        assert!(translate_goods_supply_town(b"Goods supply in ").is_none());
        assert!(translate_goods_supply_town(b"Goods supply in L\xfcbeck!").is_none());
    }

    #[test]
    fn separates_city_name_from_german_city_signature() {
        assert_eq!(
            translate_city_signature(b"Die StadtRiga")
                .unwrap()
                .as_bytes(),
            b"Die Stadt Riga"
        );
        assert_eq!(
            translate_city_signature(b"Die StadtL\xfcbeck")
                .unwrap()
                .as_bytes(),
            b"Die Stadt L\xfcbeck"
        );
        assert!(translate_city_signature(b"Die Stadtmauer").is_none());
        assert!(translate_city_signature(b"Die StadtAugsburg").is_none());
    }

    #[test]
    fn translates_dynamic_day_counts_with_german_pluralization() {
        assert_eq!(translate_day_count(b"1 day").unwrap().as_bytes(), b"1 Tag");
        assert_eq!(
            translate_day_count(b"2 days").unwrap().as_bytes(),
            b"2 Tage"
        );
        assert_eq!(
            translate_day_count(b"0 days").unwrap().as_bytes(),
            b"0 Tage"
        );
        assert!(translate_day_count(b"one day").is_none());
        assert_eq!(translate_day_count(b"1 days").unwrap().as_bytes(), b"1 Tag");
    }

    #[test]
    fn padded_translation_preserves_source_spacing() {
        let translations = RenderTranslations {
            exact: HashMap::from([(
                b"ship data".to_vec(),
                CString::new("Daten der Schiffe").unwrap(),
            )]),
            ..RenderTranslations::default()
        };
        assert_eq!(
            find_padded_render_translation(b" ship data  ", &translations)
                .unwrap()
                .as_bytes(),
            b" Daten der Schiffe  "
        );
        assert!(find_padded_render_translation(b" unknown ", &translations).is_none());
    }

    #[test]
    fn context_specific_fragments_are_not_global_render_keys() {
        let entries = load_translations().unwrap();
        let translations = load_render_translations(&entries).unwrap();
        for text in [b"to".as_slice(), b".\"", b"per", b"Name"] {
            assert!(translations.get(text).is_none());
        }
        let christen = entries
            .iter()
            .find(|entry| entry.slot_rva == 0x2B0384)
            .unwrap();
        assert_eq!(christen.source.as_bytes(), b"Name");
        assert_eq!(christen.translation.as_bytes(), b"Taufen");
        let feminine = entries
            .iter()
            .find(|entry| entry.slot_rva == 0x2A0C24)
            .unwrap();
        let masculine = entries
            .iter()
            .find(|entry| entry.slot_rva == 0x2A0C60)
            .unwrap();
        assert_eq!(feminine.source.as_bytes(), b"experienced");
        assert_eq!(masculine.source.as_bytes(), b"experienced");
        assert_eq!(feminine.translation.as_bytes(), b"erfahrene");
        assert_eq!(masculine.translation.as_bytes(), b"erfahrener");
        assert_eq!(translations.get(b"school").unwrap().as_bytes(), b"Schule");
        assert_eq!(
            translations
                .get(b"he demands 50% of the loot.")
                .unwrap()
                .as_bytes(),
            b"Er verlangt 50 % der Beute."
        );
        assert_eq!(
            translations.get(b"weekly production").unwrap().as_bytes(),
            b"Wochenproduktion"
        );
        assert_eq!(
            translations.get(b"trading information").unwrap().as_bytes(),
            b"Handelsinformationen"
        );
        assert_eq!(
            translations.get(b"no ships available").unwrap().as_bytes(),
            encode_cp1252("Keine Schiffe verfügbar", "test").unwrap()
        );
        assert_eq!(
            translations.get(b"dismiss sailors").unwrap().as_bytes(),
            b"Matrosen entlassen"
        );
        assert_eq!(
            translations.get(b"dismiss captain").unwrap().as_bytes(),
            b"Kapitain entlassen"
        );
        assert_eq!(
            translations.get(b"sea battle").unwrap().as_bytes(),
            b"Seeschlacht"
        );
        assert_eq!(
            translations.get(b"date of game").unwrap().as_bytes(),
            b"Spieldatum"
        );
        assert_eq!(
            translations.get(b"autosave").unwrap().as_bytes(),
            encode_cp1252("Automatisches Speichern", "test").unwrap()
        );
        assert_eq!(
            translations.get(b"city buildings").unwrap().as_bytes(),
            encode_cp1252("Stadtgebäude", "test").unwrap()
        );
        assert_eq!(translations.get(b"crew").unwrap().as_bytes(), b"Besatzung");
        assert_eq!(
            translations.get(b"pro load").unwrap().as_bytes(),
            b"pro Ladung"
        );
        assert_eq!(
            translations.get(b"1.000 load").unwrap().as_bytes(),
            b"1.000 Last"
        );
        assert_eq!(
            translations.get(b"2.000 load").unwrap().as_bytes(),
            b"2.000 Last"
        );
        assert_eq!(
            translations.get(b"choose mission").unwrap().as_bytes(),
            encode_cp1252("Mission wählen", "test").unwrap()
        );
        assert_eq!(translations.get(b"no").unwrap().as_bytes(), b"Nein");
        assert_eq!(
            translations.get(b"tooltips").unwrap().as_bytes(),
            b"Hinweise"
        );
        assert_eq!(translations.get(b"auction").unwrap().as_bytes(), b"Auktion");
        assert_eq!(translations.get(b"guild").unwrap().as_bytes(), b"Gilde");
        for (_, source, translation) in render_overrides()
            .unwrap()
            .into_iter()
            .filter(|(scope, _, _)| *scope == "render")
        {
            assert_eq!(
                translations
                    .get(&encode_cp1252(source, "official German name source").unwrap())
                    .unwrap()
                    .as_bytes(),
                encode_cp1252(translation, "official German name test").unwrap(),
                "official resource name was not translated: {source}"
            );
        }
        assert_eq!(
            translations
                .get(b"amtliche nachricht: elections")
                .unwrap()
                .as_bytes(),
            b"Amtliche Nachricht: Wahlen"
        );
        for (source, translation) in [
            (b"ships in port".as_slice(), b"Schiffe im Hafen".as_slice()),
            (b"ships at sea", b"Schiffe auf See"),
            (b"convoys in port", b"Konvois im Hafen"),
            (b"convoys at sea", b"Konvois auf See"),
        ] {
            assert_eq!(translations.get(source).unwrap().as_bytes(), translation);
        }
    }
}

mod renderer_tests {
    use super::*;

    #[test]
    fn embedded_table_parses_and_builds_render_translations() {
        let entries = load_translations().unwrap();
        let first = entries.first().unwrap();
        assert_eq!(first.slot_rva, 0x299868);
        assert_eq!(first.source.as_bytes(), b"Trading goods");
        assert_eq!(first.translation.as_bytes(), b"Warenhandel");
        let render = load_render_translations(&entries).unwrap();
        assert_eq!(
            render.get(b"trading goods".as_slice()).unwrap().as_bytes(),
            b"Warenhandel"
        );
        assert_eq!(render.get(b"Capacity").unwrap().as_bytes(), b"Laderaum");
        assert_eq!(
            render.get(b"capacity").unwrap().as_bytes(),
            encode_cp1252("Kapazität", "test").unwrap()
        );
        assert!(render.get(b"CAPACITY").is_none());
        assert_eq!(
            render.get(b"Town production").unwrap().as_bytes(),
            b"Stadtproduktion"
        );
    }

    #[test]
    fn translates_addon_labels_and_preserves_resource_town_names() {
        let entries = load_translations().unwrap();
        RENDER_TRANSLATIONS.get_or_init(|| load_render_translations(&entries).unwrap());
        for (source, target) in [
            ("Demolish building", "Gebäude abreißen"),
            ("Ready", "Bereit"),
            (
                " Weekly production (fixed)",
                " Wochenproduktion (korrigiert)",
            ),
            ("Public notices", "Öffentliche Nachrichten"),
            (
                "Quantity, purchase maximum",
                "Bestand, bis zu dem eingekauft wird",
            ),
            ("Utilization Markup", "Auslastungszuschlag"),
            ("Pending Experience (scaled)", "Ausstehende EP (skaliert)"),
            ("Experience (scaled)", "Erfahrung (skaliert)"),
            ("Quality Level", "Qualitätsstufe"),
            ("Required XP", "Benötigte EP"),
            (
                "Are you willing to announce a celebration?",
                "Seid Ihr willens, ein Fest anzukündigen?",
            ),
        ] {
            let actual = translate_render_text(source.as_bytes()).unwrap();
            assert_eq!(
                actual.to_bytes(),
                encode_cp1252(target, "expected").unwrap()
            );
        }
        assert!(translate_render_text(b"Cologne").is_none());
        assert!(translate_render_text(b"K\xf6ln").is_none());
        assert_eq!(
            translate_render_text(b"Information about K\xf6ln")
                .unwrap()
                .to_bytes(),
            b"Informationen \xfcber K\xf6ln",
        );
    }
}

mod regression_tests {
    use super::*;

    #[test]
    fn cp1252_supports_punctuation_and_rejects_unrepresentable_characters() {
        assert_eq!(
            encode_cp1252("€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ", "test").unwrap(),
            [
                0x80, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8e, 0x91,
                0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9e, 0x9f
            ]
        );
        assert_eq!(
            encode_cp1252("ÄÖÜäöüß", "test").unwrap(),
            [0xc4, 0xd6, 0xdc, 0xe4, 0xf6, 0xfc, 0xdf]
        );
        assert!(encode_cp1252("漢", "test").is_err());
    }

    #[test]
    fn surrender_uses_resource_identity_without_overriding_gameplay_contexts() {
        let entries = load_translations().unwrap();
        let render = load_render_translations(&entries).unwrap();
        assert!(render.get(b"Surrender").is_none());
        assert!(render.get(b"sUrReNdEr").is_none());
        assert!(render.get(b"WEALTH").is_none());
        let (source, translation) = &render.resources[&20052];
        assert_eq!(source.as_bytes(), b"Surrender");
        assert_eq!(translation.as_bytes(), b"Aufgeben");
        unsafe extern "system" fn original_resource(
            _instance: isize,
            _id: u32,
            output: *mut u8,
            capacity: i32,
        ) -> i32 {
            if capacity <= 0 || output.is_null() {
                return 0;
            }
            let source = b"Surrender";
            let length = source.len().min(capacity as usize - 1);
            std::ptr::copy_nonoverlapping(source.as_ptr(), output, length);
            *output.add(length) = 0;
            length as i32
        }
        ORIGINAL_LOAD_STRING
            .set(original_resource as *const () as usize)
            .unwrap();
        RENDER_TRANSLATIONS.get_or_init(|| load_render_translations(&entries).unwrap());
        GAME_BASE.store(0x400000, Ordering::Release);
        for (capacity, expected) in [
            (32, b"Aufgeben".as_slice()),
            (9, b"Aufgeben"),
            (4, b"Auf"),
            (1, b""),
        ] {
            let mut buffer = [0xcc; 32];
            let count = unsafe { load_string_hook(0x400000, 20052, buffer.as_mut_ptr(), capacity) };
            assert_eq!(count as usize, expected.len());
            assert_eq!(&buffer[..expected.len()], expected);
            assert_eq!(buffer[expected.len()], 0);
            assert!(buffer[capacity as usize..].iter().all(|byte| *byte == 0xcc));
        }
        let mut untouched = [0xcc; 32];
        unsafe {
            load_string_hook(0x400000, 20044, untouched.as_mut_ptr(), 32);
        }
        assert_eq!(&untouched[..9], b"Surrender");
        unsafe {
            load_string_hook(0x500000, 20052, untouched.as_mut_ptr(), 32);
        }
        assert_eq!(&untouched[..9], b"Surrender");
        assert_eq!(
            unsafe { load_string_hook(0x400000, 20052, std::ptr::null_mut(), 0) },
            0
        );
        let contextual = entries
            .iter()
            .filter(|entry| entry.source.as_bytes() == b"Surrender")
            .map(|entry| entry.translation.as_bytes())
            .collect::<HashSet<_>>();
        assert_eq!(
            contextual,
            HashSet::from([b"Kapitulation".as_slice(), b"Kapitulieren".as_slice()])
        );
    }

    #[test]
    fn player_titles_are_translated_before_formatting_for_every_rank_and_adjective() {
        let entries = load_translations().unwrap();
        let at = |slot| {
            entries
                .iter()
                .find(|entry| entry.slot_rva == slot)
                .unwrap()
                .translation
                .as_bytes()
        };
        // FUN_0047dd40 selects sex 0/1, fourteen adjectives, and eight ranks per sex.
        assert_eq!(
            [at(0x2C0EF8), at(0x2C0EFC)],
            [b"Die".as_slice(), b"Der".as_slice()]
        );
        assert_eq!(
            [at(0x2C0ED8), at(0x2C0EDC)],
            [b"die".as_slice(), b"der".as_slice()]
        );
        // Index 12 is already the German word "aggressive" and needs no patch.
        for index in (0..14).filter(|index| *index != 12) {
            let adjective = at(0x29EE48 + index * 4);
            assert!(!adjective.is_empty());
            assert_eq!(adjective, at(0x2A0C18 + index * 4));
        }
        for sex in 0..2 {
            for rank in 0..8 {
                let title = at(0x2A0608 + (sex * 8 + rank) * 4);
                assert!(!title.is_empty());
                assert!(![
                    b"shopkeeper".as_slice(),
                    b"trader",
                    b"merchant",
                    b"Councillor",
                    b"Patrician"
                ]
                .contains(&title));
            }
        }
        for slot in [0x2B3678, 0x2AA184, 0x29C86C, 0x2C2A34] {
            let format = at(slot);
            assert!(!format.windows(4).any(|part| part == b"The "));
        }
    }
}
