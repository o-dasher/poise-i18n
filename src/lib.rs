#![feature(trait_alias)]

/// This is kind of a disgusting type magic, but i think it is kind of understable
/// given the problem that this is trying to solve.
use std::{collections::HashMap, fmt::Display, str::FromStr};

use bevy_reflect::Reflect;
use itertools::{iproduct, Itertools};
use poise::{Command, CommandParameter, CommandParameterChoice};
use rusty18n::{I18NAccess, I18NFallback, I18NKey, I18NReflected, I18NTrait, I18NWrapper, R};
use strum::{Display, EnumIter, IntoEnumIterator};

pub trait PoiseI18NKey = I18NKey + FromStr + ToString + Display;

pub trait PoiseI18NMeta<'a, K: PoiseI18NKey, V: I18NFallback> {
    // Returns references to the required locales.
    fn locales(&self) -> &'a I18NWrapper<K, V>;
}

/// Automatically implemented trait for context's that provide locales.
pub trait PoiseI18NTrait<'a, K: PoiseI18NKey, V: I18NFallback> {
    // Acquires i18n access.
    fn i18n(&'a self) -> I18NAccess<I18NWrapper<K, V>>;
    fn i18n_explicit(&'a self, wrapper: &'a I18NWrapper<K, V>)
        -> I18NAccess<'a, I18NWrapper<K, V>>;
}

impl<'a, K: PoiseI18NKey + 'a, V: I18NFallback, U, E> PoiseI18NTrait<'a, K, V>
    for poise::Context<'a, U, E>
where
    Self: PoiseI18NMeta<'a, K, V>,
{
    fn i18n(&'a self) -> I18NAccess<I18NWrapper<K, V>> {
        self.i18n_explicit(self.locales())
    }

    fn i18n_explicit(
        &'a self,
        wrapper: &'a I18NWrapper<K, V>,
    ) -> I18NAccess<'a, I18NWrapper<K, V>> {
        wrapper.get(K::from_str(self.locale().unwrap_or_default()).unwrap_or_default())
    }
}

#[derive(Display, EnumIter, Clone)]
#[strum(serialize_all = "snake_case")]
enum CommandLocalization {
    Name,
    Description,
}

struct I18NAccesses<'a, L: I18NTrait>(Vec<(String, I18NAccess<'a, L>)>);

pub fn apply_translations<K: PoiseI18NKey, V: I18NFallback + Reflect, U, E>(
    commands: &mut [Command<U, E>],
    wrapper: &I18NWrapper<K, V>,
) {
    apply_translation(commands, &I18NAccesses(
wrapper
        .store
        .0
        .keys()
        .map(|key| (key.to_string(), wrapper.get(*key)))
        .collect_vec()

    ))
}

trait PoiseI18NLocalizable {
    fn name_localizations(&mut self) -> &mut HashMap<String, String>;
    fn description_localizations(&mut self) -> Option<&mut HashMap<String, String>>;
}

macro_rules! impl_localizable {
    ($struct:ident) => {
        impl<U, E> PoiseI18NLocalizable for $struct<U, E> {
            fn name_localizations(&mut self) -> &mut HashMap<String, String> {
                &mut self.name_localizations
            }

            fn description_localizations(&mut self) -> Option<&mut HashMap<String, String>> {
                Some(&mut self.description_localizations)
            }
        }
    };
}

impl_localizable!(Command);
impl_localizable!(CommandParameter);

impl PoiseI18NLocalizable for CommandParameterChoice {
    fn name_localizations(&mut self) -> &mut HashMap<String, String> {
        &mut self.localizations
    }

    fn description_localizations(&mut self) -> Option<&mut HashMap<String, String>> {
        None
    }
}

fn apply_localization<L: I18NTrait>(
    path: &mut Vec<String>,
    next_tag: String,
    localizable: &mut impl PoiseI18NLocalizable,
    locale_accesses: &I18NAccesses<'_, L>,
) where
    L::K: Display,
    L::V: Reflect,
{
    path.push(next_tag);

    let locale_tags = CommandLocalization::iter()
        .map(|l| {
            let mut path_new = path.clone();

            path_new.push(l.to_string());

            let path_string = path.iter().join(".");

            (l, path_string)
        })
        .collect_vec();

    // All combinations of locale acesses and locale tags that can
    // be used for this command.
    let permutations = iproduct!(&locale_accesses.0, &locale_tags);

    for ((lang_key, access), (locale_type, tag)) in permutations {
        let possible_resource = access.by_path::<R>(tag);

        let Some(localized_key) = possible_resource else {
            continue;
        };

        let lang_key = lang_key.clone();
        let localized_key = localized_key.clone();

        match locale_type {
            CommandLocalization::Name => {
                localizable
                    .name_localizations()
                    .insert(lang_key, localized_key);
            }
            CommandLocalization::Description => {
                match localizable.description_localizations() {
                    Some(v) => v.insert(lang_key, localized_key),
                    None => {
                        continue;
                    }
                };
            }
        };
    }
}

fn apply_translation<L: I18NTrait, U, E>(
    commands: &mut [Command<U, E>],
    locale_accesses: &I18NAccesses<L>,
) where
    L::K: Display,
    L::V: Reflect,
{
    for command in commands {
        let mut path_vec = vec![];

        // Recursive case to apply on subcommands too.
        apply_translation(&mut command.subcommands, locale_accesses);

        // This could be recursive, we could have a trait that defines Children.
        // and we keep calling apply_localization to all the children of the
        // children of the child... Yeah, you get it.
        apply_localization(
            &mut path_vec,
            command.name.clone(),
            command,
            locale_accesses,
        );

        for param in &mut command.parameters {
            apply_localization(&mut path_vec, param.name.clone(), param, locale_accesses);

            for choice in &mut param.choices {
                apply_localization(&mut path_vec, choice.name.clone(), choice, locale_accesses)
            }
        }
    }
}
