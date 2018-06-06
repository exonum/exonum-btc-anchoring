// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use exonum::helpers::fabric::{Argument, Context};

use failure;
use serde::de::DeserializeOwned;
use serde::Serialize;
use toml;

use std::collections::BTreeMap;
use std::str::FromStr;

pub trait TypedArgument {
    type ParsedType: FromStr;
    type OutputType: Serialize + DeserializeOwned + Clone + Send + Sync;

    fn name(&self) -> String;

    fn to_argument(&self) -> Argument;

    fn input_value(&self, context: &Context) -> Result<Self::OutputType, failure::Error>;

    fn input_value_to_toml(
        &self,
        context: &Context,
    ) -> Result<(String, toml::Value), failure::Error> {
        let value = toml::Value::try_from(self.input_value(context)?)?;
        Ok((self.name(), value))
    }

    fn output_value(
        &self,
        values: &BTreeMap<String, toml::Value>,
    ) -> Result<Self::OutputType, failure::Error>;
}

#[derive(Debug)]
pub struct NamedArgumentRequired<T>
where
    T: FromStr + Serialize + DeserializeOwned + Clone + Send + Sync,
    <T as FromStr>::Err: ::std::error::Error + Send + Sync + 'static,
{
    pub name: &'static str,
    pub short_key: Option<&'static str>,
    pub long_key: &'static str,
    pub help: &'static str,
    pub default: Option<T>,
}

impl<T> TypedArgument for NamedArgumentRequired<T>
where
    T: FromStr + Serialize + DeserializeOwned + Clone + Send + Sync,
    <T as FromStr>::Err: ::std::error::Error + Send + Sync + 'static,
{
    type ParsedType = T;
    type OutputType = T;

    fn name(&self) -> String {
        self.name.to_owned()
    }

    fn to_argument(&self) -> Argument {
        Argument::new_named(
            self.name,
            self.default.is_none(),
            self.help,
            self.short_key,
            self.long_key,
            false,
        )
    }

    fn input_value(&self, context: &Context) -> Result<Self::OutputType, failure::Error> {
        context
            .arg::<Self::ParsedType>(self.name)
            .ok()
            .or_else(|| self.default.clone())
            .ok_or_else(|| format_err!("Expected proper `{}` in arguments", self.long_key))
    }

    fn output_value(
        &self,
        values: &BTreeMap<String, toml::Value>,
    ) -> Result<Self::OutputType, failure::Error> {
        values
            .get(self.name)
            .ok_or_else(|| format_err!("Expected `{}` config file", self.name))?
            .clone()
            .try_into()
            .map_err(From::from)
    }
}

#[derive(Debug)]
pub struct NamedArgumentOptional<T>
where
    T: FromStr + Serialize + DeserializeOwned + Clone + Send + Sync,
    <T as FromStr>::Err: ::std::error::Error + Send + Sync + 'static,
{
    pub name: &'static str,
    pub short_key: Option<&'static str>,
    pub long_key: &'static str,
    pub help: &'static str,
    pub default: Option<T>,
}

impl<T> TypedArgument for NamedArgumentOptional<T>
where
    T: FromStr + Serialize + DeserializeOwned + Clone + Send + Sync,
    <T as FromStr>::Err: ::std::error::Error + Send + Sync + 'static,
{
    type ParsedType = T;
    type OutputType = Option<T>;

    fn name(&self) -> String {
        self.name.to_owned()
    }

    fn to_argument(&self) -> Argument {
        Argument::new_named(
            self.name,
            false,
            self.help,
            self.short_key,
            self.long_key,
            false,
        )
    }

    fn input_value(&self, context: &Context) -> Result<Self::OutputType, failure::Error> {
        Ok(context
            .arg::<Self::ParsedType>(self.name)
            .ok()
            .or_else(|| self.default.clone()))
    }

    fn output_value(
        &self,
        values: &BTreeMap<String, toml::Value>,
    ) -> Result<Self::OutputType, failure::Error> {
        values
            .get(self.name)
            .ok_or_else(|| format_err!("Expected `{}` config file", self.name))?
            .clone()
            .try_into()
            .map_err(From::from)
    }
}
