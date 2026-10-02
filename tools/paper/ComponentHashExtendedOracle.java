// Original Rustwire interoperability harness. Requires an independently obtained official server.
// Calls public component CODECs and HashOps; does not reproduce proprietary implementations.
// Compile/run with the matching release jar and its libraries on the classpath (Java 25).
// Optional arguments are a DataComponents field and JSON value, otherwise emits all fixtures.
import net.minecraft.util.HashOps;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import com.google.common.hash.HashCode;
import com.mojang.serialization.*;
import com.google.gson.*;
public class ComponentHashExtendedOracle {
 // Preserve lone Java UTF-16 surrogates in UTF-8 terminal output.
 static void print(JsonObject out) {
  var source=out.toString();var escaped=new StringBuilder();
  for(int i=0;i<source.length();i++){
   char c=source.charAt(i);
   if(Character.isHighSurrogate(c)&&i+1<source.length()&&Character.isLowSurrogate(source.charAt(i+1))){escaped.append(c).append(source.charAt(++i));}
   else if(Character.isSurrogate(c)){escaped.append(String.format("\\u%04x",(int)c));}
   else escaped.append(c);
  }
  System.out.println("ORACLE "+escaped);
 }
 @SuppressWarnings({"unchecked", "rawtypes"})
 static void emit(String name, String field, String json) {
  var out=new JsonObject(); out.addProperty("case",name); out.addProperty("component",field.toLowerCase());
  out.add("input",JsonParser.parseString(json));
  try {
   var type=(DataComponentType)DataComponents.class.getField(field).get(null);
   var value=type.codec().parse(JsonOps.INSTANCE,JsonParser.parseString(json)).getOrThrow();
   var hash=(HashCode)type.codec().encodeStart(HashOps.CRC32C_INSTANCE,value).getOrThrow();
   out.addProperty("hash",hash.asInt());
   out.addProperty("value",value.toString());
   out.addProperty("nbt",type.codec().encodeStart(net.minecraft.nbt.NbtOps.INSTANCE,value).getOrThrow().toString());
   out.add("normalized",(JsonElement)type.codec().encodeStart(JsonOps.INSTANCE,value).getOrThrow());
  } catch(Exception ex) {out.addProperty("error",ex.toString());}
  print(out);
 }
 @SuppressWarnings({"unchecked", "rawtypes"})
 static void directFireworks(int duration) {
  var value=new net.minecraft.world.item.component.Fireworks(duration,java.util.List.of());
  var out=new JsonObject();out.addProperty("case","fireworks_direct_"+duration);out.addProperty("component","fireworks");out.addProperty("duration",duration);
  out.addProperty("hash",DataComponents.FIREWORKS.codec().encodeStart(HashOps.CRC32C_INSTANCE,value).getOrThrow().asInt());
  out.addProperty("nbt",DataComponents.FIREWORKS.codec().encodeStart(net.minecraft.nbt.NbtOps.INSTANCE,value).getOrThrow().toString());
  print(out);
 }
 @SuppressWarnings({"unchecked", "rawtypes"})
 static void replaceFloat(String field, String json, String member, float number) {
  var out=new JsonObject();out.addProperty("case",field.toLowerCase()+"_"+member+"_"+Float.toString(number));out.addProperty("component",field.toLowerCase());out.addProperty("member",member);out.addProperty("float_bits",Integer.toUnsignedString(Float.floatToRawIntBits(number)));
  try {
   var type=(DataComponentType)DataComponents.class.getField(field).get(null);
   var original=type.codec().parse(JsonOps.INSTANCE,JsonParser.parseString(json)).getOrThrow();
   var members=original.getClass().getRecordComponents();var types=new Class<?>[members.length];var values=new Object[members.length];
   boolean changed=false;
   for(int i=0;i<members.length;i++){types[i]=members[i].getType();boolean selected=members[i].getName().equals(member)||(member.equals("minReach")&&members[i].getName().equals("minRange"));values[i]=selected?number:members[i].getAccessor().invoke(original);changed|=selected;}
   if(!changed)throw new IllegalArgumentException("No record member "+member);
   var value=original.getClass().getConstructor(types).newInstance(values);
   out.addProperty("hash",((HashCode)type.codec().encodeStart(HashOps.CRC32C_INSTANCE,value).getOrThrow()).asInt());
  } catch(Exception ex){out.addProperty("error",ex.toString());}
  print(out);
 }
 @SuppressWarnings({"unchecked", "rawtypes"})
 static void emitValue(String name, DataComponentType type, Object value) {
  var out=new JsonObject();out.addProperty("case",name);
  try {out.addProperty("hash",((HashCode)type.codec().encodeStart(HashOps.CRC32C_INSTANCE,value).getOrThrow()).asInt());}
  catch(Exception ex){out.addProperty("error",ex.toString());}
  print(out);
 }
 static void bookLimits() {
  for(int n:new int[]{32765,32767,32768,65535,65536}) {
   var page=net.minecraft.server.network.Filterable.<net.minecraft.network.chat.Component>passThrough(net.minecraft.network.chat.Component.literal("x".repeat(n)));
   emitValue("written_page_length_"+n,DataComponents.WRITTEN_BOOK_CONTENT,new net.minecraft.world.item.component.WrittenBookContent(net.minecraft.server.network.Filterable.passThrough("T"),"A",0,java.util.List.of(page),false));
  }
  for(int n:new int[]{100,101,256,257}) {
   net.minecraft.network.chat.Component text=net.minecraft.network.chat.Component.literal("x");
   var pages=java.util.Collections.nCopies(n,net.minecraft.server.network.Filterable.passThrough(text));
   emitValue("written_page_count_"+n,DataComponents.WRITTEN_BOOK_CONTENT,new net.minecraft.world.item.component.WrittenBookContent(net.minecraft.server.network.Filterable.passThrough("T"),"A",0,pages,false));
   try {emitValue("lore_count_"+n,DataComponents.LORE,new net.minecraft.world.item.component.ItemLore(java.util.Collections.nCopies(n,text)));}
   catch(Exception ex){var out=new JsonObject();out.addProperty("case","lore_count_"+n);out.addProperty("error",ex.toString());print(out);}
  }
 }
 public static void main(String[] args) throws Exception {
  SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
  if(args.length==2){emit("probe",args[0],args[1]); return;}
  bookLimits();
  for(float number:new float[]{-0.0f,Float.NaN,Float.POSITIVE_INFINITY,Float.NEGATIVE_INFINITY}) {
   replaceFloat("FOOD","{\"nutrition\":0,\"saturation\":0}","saturation",number);
   replaceFloat("WEAPON","{}","disableBlockingForSeconds",number);
   replaceFloat("USE_COOLDOWN","{\"seconds\":1}","seconds",number);
   replaceFloat("USE_EFFECTS","{}","speedMultiplier",number);
   replaceFloat("ATTACK_RANGE","{}","minReach",number);
  }
  for(int duration:new int[]{0,127,128,255,256,-1,-129}) {try {directFireworks(duration);}catch(Exception ex){System.out.println("ORACLE {\"case\":\"fireworks_direct_"+duration+"\",\"error\":\""+ex.getMessage()+"\"}");}}
  emit("name_plain","CUSTOM_NAME","\"Rustwire 🚀\"");
  emit("name_literal","CUSTOM_NAME","{\"text\":\"Rustwire 🚀\"}");
  emit("name_empty","CUSTOM_NAME","{\"text\":\"\"}");
  emit("name_styled","CUSTOM_NAME","{\"text\":\"Rustwire 🚀\",\"bold\":false,\"italic\":true,\"color\":\"red\",\"extra\":[\"!\",{\"text\":\"ok\"}]}");
  emit("name_hex","ITEM_NAME","{\"text\":\"color\",\"color\":\"#aBcDeF\",\"underlined\":true,\"strikethrough\":false,\"obfuscated\":false,\"insertion\":\"λ\",\"font\":\"uniform\"}");
  emit("name_translate","CUSTOM_NAME","{\"translate\":\"item.minecraft.stone\",\"fallback\":\"Rock\",\"with\":[\"a\",2,true]}");
  emit("name_list","CUSTOM_NAME","[\"a\",\"b\"]");
  emit("lore_empty","LORE","[]");
  emit("lore_plain","LORE","[\"first\",{\"text\":\"第二 🚀\",\"italic\":false}]");
  emit("food_default","FOOD","{\"nutrition\":0,\"saturation\":0}");
  emit("food_full","FOOD","{\"nutrition\":7,\"saturation\":1.25,\"can_always_eat\":true}");
  emit("food_negative","FOOD","{\"nutrition\":-1,\"saturation\":0}");
  emit("food_saturation_negative","FOOD","{\"nutrition\":1,\"saturation\":-0.5}");
  emit("cooldown_plain","USE_COOLDOWN","{\"seconds\":2.5}");
  emit("cooldown_group","USE_COOLDOWN","{\"seconds\":0.5,\"cooldown_group\":\"tools\"}");
  emit("cooldown_negative","USE_COOLDOWN","{\"seconds\":-0.5}");
  emit("weapon_default","WEAPON","{}");
  emit("weapon_full","WEAPON","{\"item_damage_per_attack\":3,\"disable_blocking_for_seconds\":2.5}");
  emit("weapon_negative","WEAPON","{\"item_damage_per_attack\":-1}");
  emit("weapon_seconds_negative","WEAPON","{\"disable_blocking_for_seconds\":-1}");
  emit("use_effects_default","USE_EFFECTS","{}");
  emit("use_effects_full","USE_EFFECTS","{\"can_sprint\":true,\"interact_vibrations\":false,\"speed_multiplier\":0.625}");
  emit("use_effects_negative","USE_EFFECTS","{\"speed_multiplier\":-0.1}");
  emit("use_effects_large","USE_EFFECTS","{\"speed_multiplier\":2.0}");
  emit("explosion_default","FIREWORK_EXPLOSION","{\"shape\":\"small_ball\"}");
  emit("explosion_full","FIREWORK_EXPLOSION","{\"shape\":\"star\",\"colors\":[-1,0,16711680],\"fade_colors\":[255],\"has_trail\":true,\"has_twinkle\":true}");
  emit("fireworks_default","FIREWORKS","{}");
  emit("fireworks_full","FIREWORKS","{\"flight_duration\":3,\"explosions\":[{\"shape\":\"burst\",\"colors\":[-2147483648,2147483647]},{\"shape\":\"creeper\"}]}");
  emit("fireworks_256","FIREWORKS","{\"flight_duration\":256}");
  emit("fireworks_negative","FIREWORKS","{\"flight_duration\":-1}");
  emit("lodestone_default","LODESTONE_TRACKER","{}");
  emit("lodestone_target","LODESTONE_TRACKER","{\"target\":{\"dimension\":\"overworld\",\"pos\":[-17,-64,30000000]},\"tracked\":false}");
  emit("writable_default","WRITABLE_BOOK_CONTENT","{}");
  emit("writable_pages","WRITABLE_BOOK_CONTENT","{\"pages\":[\"raw 🚀\",{\"raw\":\"secret\",\"filtered\":\"clean\"}]}");
  emit("written_default","WRITTEN_BOOK_CONTENT","{\"title\":\"Title\",\"author\":\"A\"}");
  emit("written_full","WRITTEN_BOOK_CONTENT","{\"title\":{\"raw\":\"Raw\",\"filtered\":\"Clean\"},\"author\":\"λ\",\"generation\":2,\"pages\":[\"hello 🚀\",{\"raw\":{\"text\":\"secret\",\"bold\":true},\"filtered\":\"clean\"}],\"resolved\":true}");
  emit("written_generation4","WRITTEN_BOOK_CONTENT","{\"title\":\"T\",\"author\":\"A\",\"generation\":4}");
  emit("written_generation_negative","WRITTEN_BOOK_CONTENT","{\"title\":\"T\",\"author\":\"A\",\"generation\":-1}");
  emit("swing_default","SWING_ANIMATION","{}");
  emit("swing_full","SWING_ANIMATION","{\"type\":\"stab\",\"duration\":9}");
  emit("swing_negative","SWING_ANIMATION","{\"duration\":-1}");
  emit("attack_range_default","ATTACK_RANGE","{}");
  emit("attack_range_full","ATTACK_RANGE","{\"min_reach\":1,\"max_reach\":4,\"min_creative_reach\":2,\"max_creative_reach\":6,\"hitbox_margin\":0.25,\"mob_factor\":0.5}");
  emit("attack_range_negative","ATTACK_RANGE","{\"min_reach\":-1}");
  emit("attack_range_large","ATTACK_RANGE","{\"max_reach\":100}");
  emit("name_zero_style","CUSTOM_NAME","{\"text\":\"x\",\"extra\":[]}");
  emit("name_shadow","CUSTOM_NAME","{\"text\":\"x\",\"shadow_color\":-123}");
  emit("food_negative_zero","FOOD","{\"nutrition\":0,\"saturation\":-0.0}");
  emit("weapon_negative_zero","WEAPON","{\"item_damage_per_attack\":1,\"disable_blocking_for_seconds\":-0.0}");
  emit("attack_range_margin_large","ATTACK_RANGE","{\"hitbox_margin\":65}");
  emit("attack_range_factor_large","ATTACK_RANGE","{\"mob_factor\":65}");
  emit("attack_range_reverse","ATTACK_RANGE","{\"min_reach\":6,\"max_reach\":1}");
  emit("swing_zero","SWING_ANIMATION","{\"duration\":0}");
  emit("consumable_inline","CONSUMABLE","{\"consume_seconds\":2.0,\"animation\":\"drink\",\"sound\":{\"sound_id\":\"entity.generic.drink\"},\"has_consume_particles\":false,\"on_consume_effects\":[{\"type\":\"clear_all_effects\"},{\"type\":\"teleport_randomly\",\"diameter\":8}]}");
  emit("death_default","DEATH_PROTECTION","{}");
  emit("death_clear","DEATH_PROTECTION","{\"death_effects\":[{\"type\":\"clear_all_effects\"}]}");
  emit("death_teleport","DEATH_PROTECTION","{\"death_effects\":[{\"type\":\"teleport_randomly\"}]}");
  emit("death_teleport8","DEATH_PROTECTION","{\"death_effects\":[{\"type\":\"teleport_randomly\",\"diameter\":8}]}");
  emit("death_sound","DEATH_PROTECTION","{\"death_effects\":[{\"type\":\"play_sound\",\"sound\":{\"sound_id\":\"block.note_block.bell\",\"range\":16}}]}");
  emit("death_teleport_negative","DEATH_PROTECTION","{\"death_effects\":[{\"type\":\"teleport_randomly\",\"diameter\":-1}]}");
  emit("sound_plain","BREAK_SOUND","{\"sound_id\":\"block.glass.break\"}");
  emit("sound_fixed","BREAK_SOUND","{\"sound_id\":\"block.glass.break\",\"range\":-1.25}");
  emit("consumable_default","CONSUMABLE","{}");
  emit("consumable_inline_default","CONSUMABLE","{\"sound\":{\"sound_id\":\"entity.generic.eat\"}}");
  emit("consumable_trident","CONSUMABLE","{\"animation\":\"trident\",\"sound\":{\"sound_id\":\"entity.generic.eat\"}}");
  emit("consumable_spear","CONSUMABLE","{\"animation\":\"spear\",\"sound\":{\"sound_id\":\"entity.generic.eat\"}}");
  emit("death_empty_apply","DEATH_PROTECTION","{\"death_effects\":[{\"type\":\"apply_effects\",\"effects\":[]}]}");
  emit("death_zero_teleport","DEATH_PROTECTION","{\"death_effects\":[{\"type\":\"teleport_randomly\",\"diameter\":0}]}");
  emit("name_named_color","CUSTOM_NAME","{\"text\":\"x\",\"color\":\"dark_red\"}");
  emit("name_reset_color","CUSTOM_NAME","{\"text\":\"x\",\"color\":\"reset\"}");
  emit("name_bad_boolean","CUSTOM_NAME","{\"text\":\"x\",\"bold\":2}");
  emit("explosion_missing_shape","FIREWORK_EXPLOSION","{}");
  emit("use_effects_zero","USE_EFFECTS","{\"speed_multiplier\":0}");
  emit("weapon_zero","WEAPON","{\"item_damage_per_attack\":0}");
  emit("fireworks_large_negative","FIREWORKS","{\"flight_duration\":-129}");
  emit("explosion_large","FIREWORK_EXPLOSION","{\"shape\":\"large_ball\"}");
  emit("swing_none","SWING_ANIMATION","{\"type\":\"none\"}");
  emit("attack_range_boundaries","ATTACK_RANGE","{\"min_reach\":64,\"max_reach\":0,\"min_creative_reach\":64,\"max_creative_reach\":0,\"hitbox_margin\":1,\"mob_factor\":2}");
  emit("consumable_zero","CONSUMABLE","{\"consume_seconds\":0,\"sound\":{\"sound_id\":\"entity.generic.eat\"}}");
  emit("consumable_negative","CONSUMABLE","{\"consume_seconds\":-1,\"sound\":{\"sound_id\":\"entity.generic.eat\"}}");
  emit("name_list_nested","CUSTOM_NAME","[{\"text\":\"a\",\"extra\":[\"b\"]},\"c\"]");
  emit("name_unpaired","CUSTOM_NAME","\"\\ud800\"");
 }
}
