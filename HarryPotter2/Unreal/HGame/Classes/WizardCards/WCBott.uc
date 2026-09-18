//===============================================================================
//  #69 Bernie Bott 
//===============================================================================

class  WCBott extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardBottTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Bottsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBottBigTexture FILE=TEXTURES\menu\Folio\Cards\Bottbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardBertieBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\69_Bertie_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardBertieBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\69_Bertie_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardBertieBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\69_Bertie_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Bertie Bott";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=69
    skin(0)=Texture'HProps.skins.WizardCardBottTex0'
	textureBig=Texture'WizCardBottBigTexture'
	strDescriptionId="WizCard_0063"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardBertieBigTextureLayer0';
	textureLayers(1)=Texture'WizCardBertieBigTextureLayer1';
	textureLayers(2)=Texture'WizCardBertieBigTextureLayer2';
}

