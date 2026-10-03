// Original synthetic protocol-fixture harness using installed release APIs.
// No game implementation code, server launch, worlds, or network connections.
// Compile with a JDK; run with one prepared Paper jar and its libraries.
import java.io.ByteArrayOutputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
public final class ContainerAuxiliaryOracle {
    // Bootstrap can redirect System.out through asynchronous logging. Keep fixtures deterministic.
    static final PrintStream report = new PrintStream(new FileOutputStream(FileDescriptor.out), true);
    static Class<?> bytebuf, buffer;
    static boolean modern;
    static Object registry;
    static Class<?> find(String... names) throws Exception {
        for (String name:names) try { return Class.forName(name); } catch(ClassNotFoundException ignored) {}
        throw new ClassNotFoundException(Arrays.toString(names));
    }
    static Method method(Class<?> owner,Class<?> result,Class<?>... args) {
        for(Method m:owner.getDeclaredMethods()) if(m.getReturnType()==result && Arrays.equals(m.getParameterTypes(),args)) { m.setAccessible(true);return m; }
        throw new IllegalStateException(owner+" missing "+result+Arrays.toString(args));
    }
    static void varint(ByteArrayOutputStream b,int n) { do { int x=n&127;n>>>=7;b.write(x|(n==0?0:128)); } while(n!=0); }
    static Object make(byte[] input) throws Exception {
        Object raw=input==null?Class.forName("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null):Class.forName("io.netty.buffer.Unpooled").getMethod("wrappedBuffer",byte[].class).invoke(null,(Object)input);
        if(modern)return buffer.getConstructor(bytebuf,Class.forName("net.minecraft.core.RegistryAccess")).newInstance(raw,registry);
        return buffer.getConstructor(bytebuf).newInstance(raw);
    }
    static Object decode(Class<?> packet,byte[] bytes) throws Exception {
        Object b=make(bytes),result;
        if(modern) { Object codec=packet.getField("STREAM_CODEC").get(null);result=Class.forName("net.minecraft.network.codec.StreamCodec").getMethod("decode",Object.class).invoke(codec,b); }
        else { Constructor<?> constructor=packet.getDeclaredConstructor(buffer);constructor.setAccessible(true);result=constructor.newInstance(b); }
        if((int)bytebuf.getMethod("readableBytes").invoke(b)!=0)throw new AssertionError("trailing bytes");return result;
    }
    static byte[] encode(Class<?> packet,Object value) throws Exception {
        Object b=make(null);
        if(modern) { Object codec=packet.getField("STREAM_CODEC").get(null);Class.forName("net.minecraft.network.codec.StreamCodec").getMethod("encode",Object.class,Object.class).invoke(codec,b,value); }
        else method(packet,void.class,buffer).invoke(value,b);
        byte[] out=new byte[(int)bytebuf.getMethod("readableBytes").invoke(b)];bytebuf.getMethod("readBytes",byte[].class).invoke(b,(Object)out);return out;
    }
    static void fixture(Class<?> packet,String label,byte[] bytes,byte[] expected) throws Exception {
        byte[] actual=encode(packet,decode(packet,bytes));if(!Arrays.equals(actual,expected))throw new AssertionError(label+" expected="+HexFormat.of().formatHex(expected)+" actual="+HexFormat.of().formatHex(actual));
        report.println(label+"="+HexFormat.of().formatHex(actual));
    }
    static void invalid(Class<?> packet,String label,byte[] bytes) throws Exception {
        try {decode(packet,bytes);throw new AssertionError("accepted "+label);}
        catch(InvocationTargetException e) {report.println(label+"=rejected:"+e.getCause().getClass().getSimpleName());}
    }
    static byte[] ints(int... values) { ByteArrayOutputStream b=new ByteArrayOutputStream();for(int n:values)varint(b,n);return b.toByteArray(); }
    static void direct(Class<?> packet,String label,int... values) throws Exception {
        Class<?>[] types=new Class<?>[values.length];Arrays.fill(types,int.class);Object[] boxed=Arrays.stream(values).boxed().toArray();
        byte[] bytes=encode(packet,packet.getConstructor(types).newInstance(boxed));
        report.println(label+"="+HexFormat.of().formatHex(bytes));
        Object decoded=decode(packet,bytes);
        report.println(label+"_roundtrip="+HexFormat.of().formatHex(encode(packet,decoded)));
        ArrayList<String> fields=new ArrayList<>();
        for(Field f:packet.getDeclaredFields()) if(f.getType()==int.class && !Modifier.isStatic(f.getModifiers())) {f.setAccessible(true);fields.add(Integer.toString(f.getInt(decoded)));}
        report.println(label+"_values="+String.join(",",fields));
    }
    static void fixed(ByteArrayOutputStream b,int n) { for(int i=3;i>=0;i--)b.write(n>>>(8*i)); }
    static void slot(ByteArrayOutputStream b,int protocol,int id,int count) {
        if(protocol<766) {b.write(1);varint(b,id);b.write(count);b.write(0);}
        else {varint(b,count);varint(b,id);b.write(0);b.write(0);}
    }
    static byte[] trades(int protocol,boolean second,int bits,int uses,int maxUses,int count,boolean component) {
        ByteArrayOutputStream b=new ByteArrayOutputStream();
        // Release packet constructors use VarInt, including 766-767 (schema mismatch).
        varint(b,7);
        varint(b,1);
        if(protocol<766)slot(b,protocol,1,count);
        else {varint(b,1);varint(b,count);varint(b,component?1:0);if(component) {varint(b,0); b.write(10);b.write(0);}}
        slot(b,protocol,1,2);
        if(protocol<766) {if(second)slot(b,protocol,1,1);else b.write(0);}
        else {b.write(second?1:0);if(second) {varint(b,1);varint(b,1);b.write(0);}}
        b.write(0);fixed(b,uses);fixed(b,maxUses);fixed(b,-2);fixed(b,-3);fixed(b,bits);fixed(b,-4);
        varint(b,-1);varint(b,-2);b.write(1);b.write(0);return b.toByteArray();
    }
    static void merchantFixture(Class<?> packet,String label,byte[] bytes,int protocol,int bits,int count) throws Exception {
        Object value=decode(packet,bytes);
        if(protocol>=766) {
            List<?> offers=(List<?>)packet.getMethod("getOffers").invoke(value);
            if(offers.size()!=1)throw new AssertionError("offer count");
            Object offer=offers.get(0);Class<?> type=offer.getClass();
            if(Float.floatToRawIntBits((float)type.getMethod("getPriceMultiplier").invoke(offer))!=bits)throw new AssertionError("price bits");
            Object cost=type.getMethod("getItemCostA").invoke(offer);
            if((int)cost.getClass().getMethod("count").invoke(cost)!=count)throw new AssertionError("cost count");
            Object result=type.getMethod("getResult").invoke(offer);
            if((int)result.getClass().getMethod("getCount").invoke(result)!=2)throw new AssertionError("result count");
            ByteArrayOutputStream expectedCost=new ByteArrayOutputStream();varint(expectedCost,1);varint(expectedCost,count);
            boolean component=label.startsWith("trades_") && label.endsWith("_true");
            varint(expectedCost,component?1:0);if(component){varint(expectedCost,0);expectedCost.write(10);expectedCost.write(0);}
            if(!Arrays.equals(encode(cost.getClass(),cost),expectedCost.toByteArray()))throw new AssertionError("cost codec mismatch");
            if((int)type.getMethod("getUses").invoke(offer)!=2 || (int)type.getMethod("getMaxUses").invoke(offer)!=10)throw new AssertionError("uses");
            if((int)type.getMethod("getXp").invoke(offer)!=-2 || (int)type.getMethod("getSpecialPriceDiff").invoke(offer)!=-3 || (int)type.getMethod("getDemand").invoke(offer)!=-4)throw new AssertionError("offer fields");
            if((int)packet.getMethod("getVillagerLevel").invoke(value)!=-1 || (int)packet.getMethod("getVillagerXp").invoke(value)!=-2)throw new AssertionError("merchant fields");
        }
        // Paper 769+ requires a running server for outbound ItemStack sanitization.
        // Decode and inspect those fixtures; do not replace or disable its sanitizer.
        if(protocol<769) {
            byte[] actual=encode(packet,value);
            if(!Arrays.equals(actual,bytes))throw new AssertionError(label+" changed during re-encode");
        }
        report.println(label+"="+HexFormat.of().formatHex(bytes));
    }
    public static void main(String[] args) throws Exception {
        try { run(args); } catch(Throwable e) {e.printStackTrace(report);System.exit(1);}
    }
    static void run(String[] args) throws Exception {
        Class<?> constants=Class.forName("net.minecraft.SharedConstants");
        try { constants.getMethod("tryDetectVersion").invoke(null); }catch(NoSuchMethodException e) { constants.getMethod("a").invoke(null); }
        Class<?> bootstrap=find("net.minecraft.server.Bootstrap","net.minecraft.server.DispenserRegistry");
        try { bootstrap.getMethod("bootStrap").invoke(null); }catch(NoSuchMethodException e) { bootstrap.getMethod("a").invoke(null); }
        int protocol=Integer.parseInt(args[0]);modern=protocol>=766;bytebuf=Class.forName("io.netty.buffer.ByteBuf");
        buffer=find(modern?"net.minecraft.network.RegistryFriendlyByteBuf":"net.minecraft.network.PacketDataSerializer");
        if(modern) registry=Class.forName("net.minecraft.core.RegistryAccess").getMethod("fromRegistryOfRegistries",Class.forName("net.minecraft.core.Registry")).invoke(null,Class.forName("net.minecraft.core.registries.BuiltInRegistries").getField("REGISTRY").get(null));
        if(protocol>=775) {
            // Isolated fixture registry context: only stone (ID 1) is used.
            // Registry-holder default components are supplied explicitly rather
            // than loading data packs or starting a server.
            Object items=Class.forName("net.minecraft.core.registries.BuiltInRegistries").getField("ITEM").get(null);
            Object holder=((Optional<?>)Class.forName("net.minecraft.core.Registry").getMethod("get",int.class).invoke(items,1)).orElseThrow();
            Class<?> map=Class.forName("net.minecraft.core.component.DataComponentMap");
            holder.getClass().getMethod("bindComponents",map).invoke(holder,map.getField("EMPTY").get(null));
        }
        Class<?> property=find("net.minecraft.network.protocol.game.ClientboundContainerSetDataPacket","net.minecraft.network.protocol.game.PacketPlayOutWindowData");
        Class<?> horse=find("net.minecraft.network.protocol.game.ClientboundHorseScreenOpenPacket","net.minecraft.network.protocol.game.ClientboundMountScreenOpenPacket","net.minecraft.network.protocol.game.PacketPlayOutOpenWindowHorse");
        Class<?> button=find("net.minecraft.network.protocol.game.ServerboundContainerButtonClickPacket","net.minecraft.network.protocol.game.PacketPlayInEnchantItem");
        Class<?> select=find("net.minecraft.network.protocol.game.ServerboundSelectTradePacket","net.minecraft.network.protocol.game.PacketPlayInTrSel");
        Class<?> cooldown=find("net.minecraft.network.protocol.game.ClientboundCooldownPacket","net.minecraft.network.protocol.game.PacketPlayOutSetCooldown");
        Class<?> merchant=find("net.minecraft.network.protocol.game.ClientboundMerchantOffersPacket","net.minecraft.network.protocol.game.PacketPlayOutOpenWindowMerchant");
        report.println("protocol="+protocol);
        for(int id:new int[]{7,255,300,-1})direct(property,"property_"+id,id,-32768,32767);
        for(int id:new int[]{7,255,300,-1})direct(horse,"horse_"+id,id,-2,Integer.MIN_VALUE);
        for(int id:new int[]{7,127,255,300,-1})direct(button,"button_"+id,id,id);
        for(int id:new int[]{0,300,-1,Integer.MIN_VALUE,Integer.MAX_VALUE})direct(select,"select_"+id,id);
        ByteArrayOutputStream cb=new ByteArrayOutputStream();if(protocol<768)varint(cb,1);else {byte[] str="minecraft:probe".getBytes(StandardCharsets.UTF_8);varint(cb,str.length);cb.writeBytes(str);}varint(cb,-1);byte[] cooldownBytes=cb.toByteArray();fixture(cooldown,"cooldown",cooldownBytes,cooldownBytes);
        for(int id:new int[]{7,255,300,-1}) {
            ByteArrayOutputStream empty=new ByteArrayOutputStream();
            varint(empty,id);
            empty.writeBytes(new byte[]{0,0,0,0,0});byte[] body=empty.toByteArray();fixture(merchant,"merchant_empty_"+id,body,body);
        }
        for(boolean second:new boolean[]{false,true})for(boolean component:new boolean[]{false,true}) {if(component&&protocol<766)continue;byte[] t=trades(protocol,second,0x3fc00000,2,10,3,component);merchantFixture(merchant,"trades_"+second+"_"+component,t,protocol,0x3fc00000,3);}
        for(int bits:new int[]{0x80000000,0xbf800000,0x7f800000,0xff800000,0x7fc01234}) {byte[] t=trades(protocol,false,bits,2,10,3,false);merchantFixture(merchant,"price_"+Integer.toHexString(bits),t,protocol,bits,3);}
        if(modern) {
            byte[] valid=trades(protocol,false,0x3fc00000,2,10,3,false);
            ByteArrayOutputStream emptyResult=new ByteArrayOutputStream();
            emptyResult.write(valid,0,5);emptyResult.write(0);emptyResult.write(valid,9,valid.length-9);
            invalid(merchant,"invalid_empty_result",emptyResult.toByteArray());
        }
        if(modern) for(int count:new int[]{0,-1,128}) {byte[] t=trades(protocol,false,0x3fc00000,2,10,count,false);merchantFixture(merchant,"cost_count_"+count,t,protocol,0x3fc00000,count);}
        if(modern) for(int count:new int[]{0,-1,128}) {byte[] cost=ints(1,count,0);fixture(Class.forName("net.minecraft.world.item.trading.ItemCost"),"cost_only_"+count,cost,cost);}
    }
}
